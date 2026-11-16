#define _GNU_SOURCE
#include <wpe/webkit.h>
#include <wpe/wpe-platform.h>
#include <wpe/headless/wpe-headless.h>
#include <json-glib/json-glib.h>
#include <xkbcommon/xkbcommon.h>
#include <glib-unix.h>
#include <sys/socket.h>
#include <sys/file.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>
#include <stdlib.h>

#define PACKET_LIMIT (8u * 1024u * 1024u)
#define ARGB8888 ((guint32)'A' | ((guint32)'R' << 8) | ((guint32)'2' << 16) | ((guint32)'4' << 24))

typedef struct Page Page;
typedef struct {
    WPEView parent;
    Page* page;
} BezelView;
typedef struct {
    WPEViewClass parent;
} BezelViewClass;
typedef struct {
    WPEDisplay parent;
    WPEDisplay* headless;
    WPEDRMDevice* device;
    WPEBufferFormats* formats;
    WPEClipboard* clipboard;
} BezelDisplay;
typedef struct {
    WPEDisplayClass parent;
} BezelDisplayClass;
typedef struct {
    WebKitInputMethodContext parent;
    Page* page;
    char* preedit;
} BezelInput;
typedef struct {
    WebKitInputMethodContextClass parent;
} BezelInputClass;
typedef struct {
    WPEClipboard parent;
} BezelClipboard;
typedef struct {
    WPEClipboardClass parent;
} BezelClipboardClass;
G_DEFINE_TYPE(BezelClipboard, bezel_clipboard, WPE_TYPE_CLIPBOARD)
G_DEFINE_TYPE(BezelView, bezel_view, WPE_TYPE_VIEW)
G_DEFINE_TYPE(BezelDisplay, bezel_display, WPE_TYPE_DISPLAY)
G_DEFINE_TYPE(BezelInput, bezel_input, WEBKIT_TYPE_INPUT_METHOD_CONTEXT)

typedef struct {
    WPEBuffer* buffer;
    guint generation;
    gboolean rendered;
} Lease;
struct Page {
    guint id, generation, sequence;
    double scale, pointer_x, pointer_y;
    gboolean software;
    WPEModifiers pressed;
    WebKitWebView* web;
    WPEToplevel* top;
    BezelView* view;
    BezelInput* input;
    GHashTable* leases;
    GPtrArray* actions;
    WebKitOptionMenu* options;
};
typedef struct {
    guint id, request, generation;
    WebKitWebView* web;
} Request;
static GMainLoop* loop;
static GHashTable* pages;
static WebKitNetworkSession* network;
static BezelDisplay* display;
static GByteArray* incoming;
static gboolean importing_clipboard;
static guint clipboard_page;

static const char* text(JsonObject* o, const char* name)
{
    return json_object_has_member(o, name) ? json_object_get_string_member(o, name) : "";
}
static double number(JsonObject* o, const char* name)
{
    return json_object_has_member(o, name) ? json_object_get_double_member(o, name) : 0;
}
static JsonObject* object(void) { return json_object_new(); }
static void integer(JsonObject* o, const char* name, gint64 n)
{
    json_object_set_int_member(o, name, n);
}
static void string(JsonObject* o, const char* name, const char* s)
{
    json_object_set_string_member(o, name, s ? s : "");
}
static void boolean(JsonObject* o, const char* name, gboolean value)
{
    json_object_set_boolean_member(o, name, value);
}
static void write_bytes(const void* bytes, gsize length)
{
    const char* cursor = bytes;
    while (length) {
        ssize_t count = send(0, cursor, length, MSG_NOSIGNAL);
        if (count < 0 && errno == EINTR)
            continue;
        if (count <= 0)
            exit(0);
        cursor += count;
        length -= count;
    }
}
static void packet(char type, guint id, const void* bytes, gsize size, int fd)
{
    if (size > PACKET_LIMIT)
        exit(2);
    guint8 header[9] = { type };
    guint32 page = GUINT32_TO_LE(id), length = GUINT32_TO_LE(size);
    memcpy(header + 1, &page, 4);
    memcpy(header + 5, &length, 4);
    if (fd >= 0) {
        union {
            struct cmsghdr align;
            char bytes[CMSG_SPACE(sizeof(int))];
        } control = { 0 };
        struct iovec io = { header, sizeof header };
        struct msghdr message = { .msg_iov = &io,
            .msg_iovlen = 1,
            .msg_control = control.bytes,
            .msg_controllen = sizeof control.bytes };
        struct cmsghdr* rights = CMSG_FIRSTHDR(&message);
        rights->cmsg_level = SOL_SOCKET;
        rights->cmsg_type = SCM_RIGHTS;
        rights->cmsg_len = CMSG_LEN(sizeof fd);
        memcpy(CMSG_DATA(rights), &fd, sizeof fd);
        ssize_t count;
        do {
            count = sendmsg(0, &message, MSG_NOSIGNAL);
        } while (count < 0 && errno == EINTR);
        if (count <= 0)
            exit(0);
        write_bytes(header + count, sizeof header - count);
    } else
        write_bytes(header, sizeof header);
    write_bytes(bytes, size);
}
static void json_packet(char type, guint id, JsonObject* o, int fd)
{
    JsonNode* node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, o);
    char* json = json_to_string(node, FALSE);
    packet(type, id, json, strlen(json), fd);
    g_free(json);
    json_node_free(node);
}
static void failure(guint id, const char* message)
{
    packet('E', id, message, strlen(message), -1);
}
static void status(Page* page, const char* error)
{
    JsonObject* o = object();
    string(o, "url", webkit_web_view_get_uri(page->web));
    string(o, "title", webkit_web_view_get_title(page->web));
    boolean(o, "loading", webkit_web_view_is_loading(page->web));
    boolean(o, "can_back", webkit_web_view_can_go_back(page->web));
    boolean(o, "can_forward", webkit_web_view_can_go_forward(page->web));
    if (error)
        string(o, "error", error);
    json_packet('S', page->id, o, -1);
}
static int pixel_file(const guint8* pixels, int width, int height, guint stride)
{
    if (width <= 0 || height <= 0 || width > 8192 || height > 8192 || stride < (guint)width * 4)
        return -1;
    int fd = memfd_create("bezel-wpe-frame", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (fd < 0)
        return -1;
    gsize row = width * 4;
    for (int y = 0; y < height; y++) {
        gsize written = 0;
        while (written < row) {
            ssize_t count = write(fd, pixels + y * (gsize)stride + written, row - written);
            if (count < 0 && errno == EINTR)
                continue;
            if (count <= 0) {
                close(fd);
                return -1;
            }
            written += count;
        }
    }
    if (fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE | F_SEAL_SEAL) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}
static JsonObject* buffer_info(guint token, int width, int height)
{
    JsonObject* o = object();
    integer(o, "buffer", token);
    integer(o, "width", width);
    integer(o, "height", height);
    integer(o, "fourcc", ARGB8888);
    integer(o, "stride", width * 4);
    integer(o, "offset", 0);
    string(o, "kind", "shm");
    return o;
}
static gboolean export_frame(Page* page, guint token, Lease* lease, GError** error)
{
    WPEBuffer* buffer = lease->buffer;
    int width = wpe_buffer_get_width(buffer), height = wpe_buffer_get_height(buffer);
    JsonObject* o = buffer_info(token, width, height);
    int fd = -1;
    gboolean copied = TRUE;
    if (!page->software && WPE_IS_BUFFER_DMA_BUF(buffer)) {
        WPEBufferDMABuf* dma = WPE_BUFFER_DMA_BUF(buffer);
        if (wpe_buffer_dma_buf_get_n_planes(dma) == 1
            && wpe_buffer_dma_buf_get_format(dma) == ARGB8888) {
            copied = FALSE;
            fd = wpe_buffer_dma_buf_get_fd(dma, 0);
            string(o, "kind", "dmabuf");
            integer(o, "stride", wpe_buffer_dma_buf_get_stride(dma, 0));
            integer(o, "offset", wpe_buffer_dma_buf_get_offset(dma, 0));
            integer(o, "modifier", (gint64)wpe_buffer_dma_buf_get_modifier(dma));
        }
    }
    if (copied) {
        GBytes* pixels = wpe_buffer_import_to_pixels(buffer, error);
        if (pixels && g_bytes_get_size(pixels) >= (gsize)width * height * 4)
            fd = pixel_file(g_bytes_get_data(pixels, NULL), width, height, width * 4);
    }
    if (fd < 0) {
        json_object_unref(o);
        if (error && !*error)
            g_set_error_literal(
                error, WPE_BUFFER_ERROR, WPE_BUFFER_ERROR_IMPORT_FAILED, "Cannot export WPE frame");
        return FALSE;
    }
    json_packet('B', page->id, o, fd);
    if (copied)
        close(fd);
    o = object();
    integer(o, "buffer", token);
    integer(o, "generation", lease->generation);
    json_object_set_double_member(o, "scale", page->scale);
    json_packet('F', page->id, o, -1);
    return TRUE;
}
static gboolean render(
    WPEView* view, WPEBuffer* buffer, const WPERectangle* damage, guint n_damage, GError** error)
{
    Page* page = ((BezelView*)view)->page;
    if (!page || page->sequence == G_MAXUINT)
        return FALSE;
    guint token = ++page->sequence;
    Lease* lease = g_new0(Lease, 1);
    lease->buffer = g_object_ref(buffer);
    lease->generation = page->generation;
    g_hash_table_insert(page->leases, GUINT_TO_POINTER(token), lease);
    if (export_frame(page, token, lease, error))
        return TRUE;
    g_hash_table_remove(page->leases, GUINT_TO_POINTER(token));
    return FALSE;
}
static void attached(WPEView* view, GParamSpec* property, gpointer data)
{
    if (wpe_view_get_toplevel(view))
        wpe_view_map(view);
    else
        wpe_view_unmap(view);
}
static void cursor(WPEView* view, const char* name)
{
    Page* page = ((BezelView*)view)->page;
    if (page)
        packet('K', page->id, name, strlen(name), -1);
}
static void bezel_view_init(BezelView* view)
{
    g_signal_connect(view, "notify::toplevel", G_CALLBACK(attached), NULL);
}
static void bezel_view_class_init(BezelViewClass* klass)
{
    WPE_VIEW_CLASS(klass)->render_buffer = render;
    WPE_VIEW_CLASS(klass)->set_cursor_from_name = cursor;
}
static gboolean connect_display(WPEDisplay* base, GError** error)
{
    BezelDisplay* self = (BezelDisplay*)base;
    self->clipboard = g_object_new(bezel_clipboard_get_type(), "display", base, NULL);
    self->headless = wpe_display_headless_new();
    return wpe_display_connect(self->headless, error);
}
static WPEView* create_view(WPEDisplay* base)
{
    return g_object_new(bezel_view_get_type(), "display", base, NULL);
}
static WPEToplevel* create_top(WPEDisplay* base, guint maximum)
{
    return g_object_new(WPE_TYPE_TOPLEVEL_HEADLESS, "display", base, "max-views", maximum, NULL);
}
static gpointer egl_display(WPEDisplay* base, GError** error)
{
    return wpe_display_get_egl_display(((BezelDisplay*)base)->headless, error);
}
static WPEClipboard* get_clipboard(WPEDisplay* base) { return ((BezelDisplay*)base)->clipboard; }
static WPEDRMDevice* drm_device(WPEDisplay* base) { return ((BezelDisplay*)base)->device; }
static WPEBufferFormats* formats(WPEDisplay* base) { return ((BezelDisplay*)base)->formats; }
static void display_dispose(GObject* object)
{
    BezelDisplay* self = (BezelDisplay*)object;
    g_clear_object(&self->headless);
    g_clear_object(&self->clipboard);
    g_clear_pointer(&self->device, wpe_drm_device_unref);
    g_clear_object(&self->formats);
    G_OBJECT_CLASS(bezel_display_parent_class)->dispose(object);
}
static void bezel_display_init(BezelDisplay* self) { }
static void bezel_display_class_init(BezelDisplayClass* klass)
{
    WPEDisplayClass* base = WPE_DISPLAY_CLASS(klass);
    base->connect = connect_display;
    base->create_view = create_view;
    base->create_toplevel = create_top;
    base->get_egl_display = egl_display;
    base->get_clipboard = get_clipboard;
    base->get_drm_device = drm_device;
    base->get_preferred_buffer_formats = formats;
    G_OBJECT_CLASS(klass)->dispose = display_dispose;
}
static void input_state(BezelInput* input, JsonObject* state)
{
    if (input->page)
        json_packet('I', input->page->id, state, -1);
    else
        json_object_unref(state);
}
static void preedit(WebKitInputMethodContext* base, char** text, GList** underlines, guint* offset)
{
    BezelInput* input = (BezelInput*)base;
    const char* value = input->preedit ? input->preedit : "";
    guint cursor = g_utf8_strlen(value, -1);
    if (text)
        *text = g_strdup(value);
    if (underlines)
        *underlines
            = *value ? g_list_append(NULL, webkit_input_method_underline_new(0, cursor)) : NULL;
    if (offset)
        *offset = cursor;
}
static void input_focus(WebKitInputMethodContext* base)
{
    JsonObject* o = object();
    boolean(o, "focused", TRUE);
    input_state((BezelInput*)base, o);
}
static void input_blur(WebKitInputMethodContext* base)
{
    JsonObject* o = object();
    boolean(o, "focused", FALSE);
    input_state((BezelInput*)base, o);
}
static void caret(WebKitInputMethodContext* base, int x, int y, int width, int height)
{
    JsonObject* o = object();
    JsonArray* rect = json_array_new();
    json_array_add_int_element(rect, x);
    json_array_add_int_element(rect, y);
    json_array_add_int_element(rect, width);
    json_array_add_int_element(rect, height);
    json_object_set_array_member(o, "caret", rect);
    input_state((BezelInput*)base, o);
}
static void surrounding(
    WebKitInputMethodContext* base, const char* value, guint length, guint cursor, guint selection)
{
    JsonObject* o = object();
    char* copy = g_strndup(value, length);
    string(o, "text", copy);
    integer(o, "cursor", cursor);
    integer(o, "selection", selection);
    input_state((BezelInput*)base, o);
    g_free(copy);
}
static void input_reset(WebKitInputMethodContext* base)
{
    BezelInput* input = (BezelInput*)base;
    if (input->preedit && *input->preedit) {
        g_clear_pointer(&input->preedit, g_free);
        g_signal_emit_by_name(input, "preedit-changed");
        g_signal_emit_by_name(input, "preedit-finished");
    }
}
static void input_dispose(GObject* object)
{
    g_clear_pointer(&((BezelInput*)object)->preedit, g_free);
    G_OBJECT_CLASS(bezel_input_parent_class)->dispose(object);
}
static void bezel_input_init(BezelInput* input) { }
static void bezel_input_class_init(BezelInputClass* klass)
{
    WebKitInputMethodContextClass* base = WEBKIT_INPUT_METHOD_CONTEXT_CLASS(klass);
    base->get_preedit = preedit;
    base->notify_focus_in = input_focus;
    base->notify_focus_out = input_blur;
    base->notify_cursor_area = caret;
    base->notify_surrounding = surrounding;
    base->reset = input_reset;
    G_OBJECT_CLASS(klass)->dispose = input_dispose;
}
static void lease_free(gpointer data)
{
    Lease* lease = data;
    g_object_unref(lease->buffer);
    g_free(lease);
}
static void dismiss(Page* page)
{
    if (page->options)
        webkit_option_menu_close(page->options);
    g_clear_object(&page->options);
    if (page->actions)
        g_ptr_array_set_size(page->actions, 0);
}
static void page_free(gpointer data)
{
    Page* page = data;
    page->view->page = NULL;
    page->input->page = NULL;
    dismiss(page);
    g_ptr_array_unref(page->actions);
    wpe_view_set_toplevel(WPE_VIEW(page->view), NULL);
    g_signal_handlers_disconnect_by_data(page->web, page);
    g_signal_handlers_disconnect_by_data(webkit_web_view_get_user_content_manager(page->web), page);
    g_object_unref(page->web);
    g_object_unref(page->input);
    g_object_unref(page->top);
    g_hash_table_unref(page->leases);
    g_free(page);
}
static void changed(WebKitWebView* web, GParamSpec* property, Page* page) { status(page, NULL); }
static void loaded(WebKitWebView* web, WebKitLoadEvent event, Page* page) { status(page, NULL); }
static gboolean load_failed(
    WebKitWebView* web, WebKitLoadEvent event, const char* uri, GError* error, Page* page)
{
    status(page, error->message);
    return FALSE;
}
static void terminated(WebKitWebView* web, WebKitWebProcessTerminationReason reason, Page* page)
{
    failure(page->id, "WPE web process terminated; reload the page");
}
static WebKitWebView* opened(WebKitWebView* web, WebKitNavigationAction* action, Page* page)
{
    const char* uri = webkit_uri_request_get_uri(webkit_navigation_action_get_request(action));
    packet('N', page->id, uri, strlen(uri), -1);
    return NULL;
}
static void script_message(WebKitUserContentManager* manager, JSCValue* value, Page* page)
{
    char* body = jsc_value_to_string(value);
    if (!strcmp(body, "moved"))
        status(page, NULL);
    else
        packet('L', page->id, body, strlen(body), -1);
    g_free(body);
}
static Request* request(Page* page, guint id)
{
    Request* req = g_new0(Request, 1);
    req->id = page->id;
    req->request = id;
    req->generation = page->generation;
    req->web = g_object_ref(page->web);
    return req;
}
static Page* request_page(Request* req)
{
    Page* page = g_hash_table_lookup(pages, GUINT_TO_POINTER(req->id));
    return page && page->web == req->web ? page : NULL;
}
static void request_free(Request* req)
{
    g_object_unref(req->web);
    g_free(req);
}
static void evaluated(GObject* source, GAsyncResult* result, gpointer data)
{
    Request* req = data;
    GError* error = NULL;
    JSCValue* value
        = webkit_web_view_evaluate_javascript_finish(WEBKIT_WEB_VIEW(source), result, &error);
    if (request_page(req)) {
        JsonObject* o = object();
        integer(o, "request", req->request);
        char* json = value ? jsc_value_to_json(value, 0) : NULL;
        string(o, "value", json ? json : "null");
        json_packet('J', req->id, o, -1);
        g_free(json);
    }
    g_clear_object(&value);
    g_clear_error(&error);
    request_free(req);
}
static void snapshot(GObject* source, GAsyncResult* result, gpointer data)
{
    Request* req = data;
    GError* error = NULL;
    WebKitImage* image
        = webkit_web_view_get_snapshot_finish(WEBKIT_WEB_VIEW(source), result, &error);
    Page* page = request_page(req);
    int fd = -1;
    if (page && page->generation == req->generation && image) {
        GBytes* pixels = webkit_image_as_bytes(image);
        fd = pixel_file(g_bytes_get_data(pixels, NULL), webkit_image_get_width(image),
            webkit_image_get_height(image), webkit_image_get_stride(image));
    }
    if (page) {
        JsonObject* o = fd >= 0 ? buffer_info(req->request, webkit_image_get_width(image),
                                      webkit_image_get_height(image))
                                : object();
        integer(o, "request", req->request);
        json_packet(fd >= 0 ? 'P' : 'Q', req->id, o, fd);
    }
    if (fd >= 0)
        close(fd);
    g_clear_object(&image);
    g_clear_error(&error);
    request_free(req);
}
static void clipboard_changed(
    WPEClipboard* clipboard, GPtrArray* formats, gboolean local, WPEClipboardContent* content)
{
    if (importing_clipboard || !local)
        return;
    char* value = wpe_clipboard_read_text(clipboard, "text/plain;charset=utf-8", NULL);
    if (value) {
        packet('C', clipboard_page, value, strlen(value), -1);
        g_free(value);
    }
}
static void bezel_clipboard_init(BezelClipboard* clipboard) { }
static void bezel_clipboard_class_init(BezelClipboardClass* klass)
{
    WPE_CLIPBOARD_CLASS(klass)->changed = clipboard_changed;
}
static void menu_item(
    JsonArray* array, const char* label, const char* action, gboolean enabled, gboolean selected)
{
    JsonObject* o = object();
    string(o, "label", label);
    string(o, "action", action);
    boolean(o, "enabled", enabled);
    boolean(o, "selected", selected);
    json_array_add_object_element(array, o);
}
static gboolean context_menu(
    WebKitWebView* web, WebKitContextMenu* menu, WebKitHitTestResult* hit, Page* page)
{
    dismiss(page);
    JsonObject* o = object();
    JsonArray* items = json_array_new();
    for (GList* it = webkit_context_menu_get_items(menu); it; it = it->next) {
        WebKitContextMenuItem* item = it->data;
        GAction* action = webkit_context_menu_item_get_gaction(item);
        if (!action || webkit_context_menu_item_is_separator(item)
            || webkit_context_menu_item_get_submenu(item))
            continue;
        g_ptr_array_add(page->actions, g_object_ref(item));
        char* command = g_strdup_printf("menu-%u", page->actions->len - 1);
        GVariant* state = g_action_get_state(action);
        gboolean selected = state && g_variant_is_of_type(state, G_VARIANT_TYPE_BOOLEAN)
            && g_variant_get_boolean(state);
        menu_item(items, webkit_context_menu_item_get_title(item), command,
            g_action_get_enabled(action), selected);
        if (state)
            g_variant_unref(state);
        g_free(command);
    }
    json_object_set_double_member(o, "x", page->pointer_x);
    json_object_set_double_member(o, "y", page->pointer_y);
    json_object_set_array_member(o, "items", items);
    json_packet('M', page->id, o, -1);
    return TRUE;
}
static gboolean options(
    WebKitWebView* web, WebKitOptionMenu* menu, WebKitRectangle* rect, Page* page)
{
    dismiss(page);
    page->options = g_object_ref(menu);
    JsonObject* o = object();
    JsonArray* items = json_array_new();
    for (guint i = 0; i < webkit_option_menu_get_n_items(menu); i++) {
        WebKitOptionMenuItem* item = webkit_option_menu_get_item(menu, i);
        char* command = g_strdup_printf("option-%u", i);
        menu_item(items, webkit_option_menu_item_get_label(item), command,
            webkit_option_menu_item_is_enabled(item)
                && !webkit_option_menu_item_is_group_label(item),
            webkit_option_menu_item_is_selected(item));
        g_free(command);
    }
    integer(o, "x", rect->x);
    integer(o, "y", rect->y + rect->height);
    json_object_set_array_member(o, "items", items);
    json_packet('M', page->id, o, -1);
    return TRUE;
}
static void create_page(guint id, JsonObject* command)
{
    if (!id || g_hash_table_contains(pages, GUINT_TO_POINTER(id)))
        return;
    Page* page = g_new0(Page, 1);
    page->id = id;
    page->scale = 1;
    page->leases = g_hash_table_new_full(g_direct_hash, g_direct_equal, NULL, lease_free);
    page->actions = g_ptr_array_new_with_free_func(g_object_unref);
    WebKitUserContentManager* content = webkit_user_content_manager_new();
    g_signal_connect(content, "script-message-received::ipc", G_CALLBACK(script_message), page);
    webkit_user_content_manager_register_script_message_handler(content, "ipc", NULL);
    WebKitUserScript* script
        = webkit_user_script_new(text(command, "script"), WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
            WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START, NULL, NULL);
    webkit_user_content_manager_add_script(content, script);
    webkit_user_script_unref(script);
    page->web = g_object_new(WEBKIT_TYPE_WEB_VIEW, "display", display, "network-session", network,
        "user-content-manager", content, NULL);
    g_object_unref(content);
    page->view = (BezelView*)webkit_web_view_get_wpe_view(page->web);
    page->view->page = page;
    page->top = wpe_display_create_toplevel(WPE_DISPLAY(display), 1);
    page->input = g_object_new(bezel_input_get_type(), NULL);
    page->input->page = page;
    webkit_web_view_set_input_method_context(page->web, WEBKIT_INPUT_METHOD_CONTEXT(page->input));
    if (*text(command, "userAgent"))
        webkit_settings_set_user_agent(
            webkit_web_view_get_settings(page->web), text(command, "userAgent"));
    g_hash_table_insert(pages, GUINT_TO_POINTER(id), page);
    wpe_view_set_toplevel(WPE_VIEW(page->view), page->top);
    wpe_view_resized(WPE_VIEW(page->view), 800, 600);
    g_signal_connect(page->web, "notify::uri", G_CALLBACK(changed), page);
    g_signal_connect(page->web, "notify::title", G_CALLBACK(changed), page);
    g_signal_connect(page->web, "notify::is-loading", G_CALLBACK(changed), page);
    g_signal_connect(page->web, "load-changed", G_CALLBACK(loaded), page);
    g_signal_connect(page->web, "load-failed", G_CALLBACK(load_failed), page);
    g_signal_connect(page->web, "web-process-terminated", G_CALLBACK(terminated), page);
    g_signal_connect(page->web, "create", G_CALLBACK(opened), page);
    g_signal_connect(page->web, "context-menu", G_CALLBACK(context_menu), page);
    g_signal_connect(page->web, "show-option-menu", G_CALLBACK(options), page);
}
static void store_cleared(GObject* source, GAsyncResult* result, gpointer data)
{
    GError* error = NULL;
    gboolean ok = webkit_website_data_manager_clear_finish(
        WEBKIT_WEBSITE_DATA_MANAGER(source), result, &error);
    JsonObject* o = object();
    boolean(o, "ok", ok);
    if (error)
        string(o, "error", error->message);
    json_packet('D', GPOINTER_TO_UINT(data), o, -1);
    g_clear_error(&error);
}
static void store_fetched(GObject* source, GAsyncResult* result, gpointer data)
{
    GError* error = NULL;
    GList* entries = webkit_website_data_manager_fetch_finish(
        WEBKIT_WEBSITE_DATA_MANAGER(source), result, &error);
    JsonObject* o = object();
    JsonArray* origins = json_array_new();
    guint64 total = 0;
    for (GList* it = entries; it; it = it->next) {
        WebKitWebsiteData* entry = it->data;
        JsonObject* origin = object();
        guint64 bytes = webkit_website_data_get_size(entry, WEBKIT_WEBSITE_DATA_ALL);
        string(origin, "origin", webkit_website_data_get_name(entry));
        integer(origin, "bytes", bytes);
        json_array_add_object_element(origins, origin);
        total += bytes;
    }
    JsonArray* sites = json_array_new();
    for (GList* it = entries; it; it = it->next)
        json_array_add_string_element(sites, webkit_website_data_get_name(it->data));
    json_object_set_array_member(o, "sites", sites);
    integer(o, "bytes", total);
    json_object_set_array_member(o, "origins", origins);
    if (error)
        string(o, "error", error->message);
    json_packet('D', GPOINTER_TO_UINT(data), o, -1);
    g_list_free_full(entries, (GDestroyNotify)webkit_website_data_unref);
    g_clear_error(&error);
}
static void event(Page* page, WPEEvent* event)
{
    wpe_view_event(WPE_VIEW(page->view), event);
    wpe_event_unref(event);
}
static WPEModifiers modifiers(guint flags)
{
    return ((flags & 1) ? WPE_MODIFIER_KEYBOARD_SHIFT : 0)
        | ((flags & 4) ? WPE_MODIFIER_KEYBOARD_CONTROL : 0)
        | ((flags & 8) ? WPE_MODIFIER_KEYBOARD_ALT : 0)
        | ((flags & (1u << 26)) ? WPE_MODIFIER_KEYBOARD_META : 0);
}
static void command(JsonObject* o)
{
    guint id = number(o, "id");
    const char* cmd = text(o, "cmd");
    WebKitWebsiteDataManager* manager = webkit_network_session_get_website_data_manager(network);
    if (!strcmp(cmd, "store-clear")) {
        webkit_website_data_manager_clear(
            manager, WEBKIT_WEBSITE_DATA_ALL, 0, NULL, store_cleared, GUINT_TO_POINTER(id));
        return;
    }
    if (!strcmp(cmd, "store-usage")) {
        webkit_website_data_manager_fetch(
            manager, WEBKIT_WEBSITE_DATA_ALL, NULL, store_fetched, GUINT_TO_POINTER(id));
        return;
    }
    if (!strcmp(cmd, "create")) {
        create_page(id, o);
        return;
    }
    Page* page = g_hash_table_lookup(pages, GUINT_TO_POINTER(id));
    if (!page)
        return;
    if (!strcmp(cmd, "copy") || !strcmp(cmd, "cut") || !strcmp(cmd, "paste") || !strcmp(cmd, "down")
        || (!strcmp(cmd, "focus") && number(o, "value")))
        clipboard_page = id;
    if (!strcmp(cmd, "close")) {
        g_hash_table_remove(pages, GUINT_TO_POINTER(id));
        return;
    }
    if (!strcmp(cmd, "load"))
        webkit_web_view_load_uri(page->web, text(o, "url"));
    else if (!strcmp(cmd, "back"))
        webkit_web_view_go_back(page->web);
    else if (!strcmp(cmd, "forward"))
        webkit_web_view_go_forward(page->web);
    else if (!strcmp(cmd, "reload"))
        webkit_web_view_reload(page->web);
    else if (!strcmp(cmd, "reload-bypass"))
        webkit_web_view_reload_bypass_cache(page->web);
    else if (!strcmp(cmd, "eval"))
        webkit_web_view_evaluate_javascript(page->web, text(o, "script"), -1, NULL, NULL, NULL,
            evaluated, request(page, number(o, "request")));
    else if (!strcmp(cmd, "snapshot"))
        webkit_web_view_get_snapshot(page->web, WEBKIT_SNAPSHOT_REGION_VISIBLE,
            WEBKIT_SNAPSHOT_OPTIONS_NONE, NULL, snapshot, request(page, number(o, "request")));
    else if (!strcmp(cmd, "resize")) {
        int width = number(o, "width"), height = number(o, "height");
        double scale = number(o, "scale");
        if (width < 1 || height < 1 || scale < 0.5 || scale > 4 || width * scale > 8192
            || height * scale > 8192) {
            failure(id, "Invalid page viewport");
            return;
        }
        page->generation = number(o, "generation");
        page->scale = scale;
        wpe_toplevel_scale_changed(page->top, scale);
        wpe_toplevel_resize(page->top, width, height);
        wpe_view_resized(WPE_VIEW(page->view), width, height);
    } else if (!strcmp(cmd, "visible")) {
        if (number(o, "value"))
            wpe_view_map(WPE_VIEW(page->view));
        else
            wpe_view_unmap(WPE_VIEW(page->view));
    } else if (!strcmp(cmd, "focus")) {
        if (number(o, "value"))
            wpe_view_focus_in(WPE_VIEW(page->view));
        else {
            page->pressed = 0;
            wpe_view_focus_out(WPE_VIEW(page->view));
        }
    } else if (!strcmp(cmd, "active"))
        wpe_toplevel_state_changed(
            page->top, number(o, "value") ? WPE_TOPLEVEL_STATE_ACTIVE : WPE_TOPLEVEL_STATE_NONE);
    else if (!strcmp(cmd, "presented") || !strcmp(cmd, "released") || !strcmp(cmd, "readback")) {
        guint token = number(o, "buffer");
        Lease* lease = g_hash_table_lookup(page->leases, GUINT_TO_POINTER(token));
        if (!lease)
            return;
        if (!strcmp(cmd, "readback")) {
            page->software = TRUE;
            GError* error = NULL;
            if (!export_frame(page, token, lease, &error))
                failure(id, error ? error->message : "WPE pixel import failed");
            g_clear_error(&error);
            return;
        }
        if (!lease->rendered) {
            lease->rendered = TRUE;
            wpe_view_buffer_rendered(WPE_VIEW(page->view), lease->buffer);
        }
        if (!strcmp(cmd, "released")) {
            wpe_view_buffer_released(WPE_VIEW(page->view), lease->buffer);
            g_hash_table_remove(page->leases, GUINT_TO_POINTER(token));
            JsonObject* gone = object();
            integer(gone, "buffer", token);
            json_packet('X', id, gone, -1);
        }
    } else if (!strcmp(cmd, "clipboard")) {
        WPEClipboardContent* content = wpe_clipboard_content_new();
        wpe_clipboard_content_set_text(content, text(o, "text"));
        importing_clipboard = TRUE;
        wpe_clipboard_set_content(wpe_display_get_clipboard(WPE_DISPLAY(display)), content);
        importing_clipboard = FALSE;
        wpe_clipboard_content_unref(content);
    } else if (!strcmp(cmd, "preedit") || !strcmp(cmd, "commit") || !strcmp(cmd, "unmark")) {
        BezelInput* input = page->input;
        if (number(o, "replaceLength"))
            g_signal_emit_by_name(input, "delete-surrounding", (int)number(o, "replaceOffset"),
                (guint)number(o, "replaceLength"));
        gboolean was_marked = input->preedit && *input->preedit;
        const char* value
            = !strcmp(cmd, "unmark") ? (input->preedit ? input->preedit : "") : text(o, "text");
        if (!strcmp(cmd, "preedit")) {
            if (!was_marked && *value)
                g_signal_emit_by_name(input, "preedit-started");
            g_free(input->preedit);
            input->preedit = g_strdup(value);
            g_signal_emit_by_name(input, "preedit-changed");
            if (was_marked && !*value)
                g_signal_emit_by_name(input, "preedit-finished");
        } else {
            if (was_marked || strcmp(cmd, "unmark"))
                g_signal_emit_by_name(input, "committed", value);
            g_clear_pointer(&input->preedit, g_free);
            if (was_marked) {
                g_signal_emit_by_name(input, "preedit-changed");
                g_signal_emit_by_name(input, "preedit-finished");
            }
        }
    } else if (!strcmp(cmd, "key_down") || !strcmp(cmd, "key_up")) {
        xkb_keysym_t key = xkb_keysym_from_name(text(o, "key"), XKB_KEYSYM_CASE_INSENSITIVE);
        if (!key && *text(o, "key"))
            key = xkb_utf32_to_keysym(g_utf8_get_char(text(o, "key")));
        WPEKeymapEntry* entries = NULL;
        guint count = 0, code = 0;
        if (wpe_keymap_get_entries_for_keyval(
                wpe_display_get_keymap(WPE_DISPLAY(display)), key, &entries, &count)
            && count)
            code = entries[0].keycode;
        g_free(entries);
        event(page,
            wpe_event_keyboard_new(
                !strcmp(cmd, "key_down") ? WPE_EVENT_KEYBOARD_KEY_DOWN : WPE_EVENT_KEYBOARD_KEY_UP,
                WPE_VIEW(page->view), WPE_INPUT_SOURCE_KEYBOARD, g_get_monotonic_time() / 1000,
                modifiers(number(o, "mods")), code, key));
    } else if ((!strcmp(cmd, "down") || !strcmp(cmd, "up") || !strcmp(cmd, "move")
                   || !strcmp(cmd, "enter") || !strcmp(cmd, "leave"))
        || !strcmp(cmd, "scroll")) {
        double x = number(o, "x"), y = number(o, "y");
        guint32 time = g_get_monotonic_time() / 1000;
        WPEModifiers mods = modifiers(number(o, "mods")) | page->pressed;
        if (!strcmp(cmd, "scroll"))
            event(page,
                wpe_event_scroll_new(WPE_VIEW(page->view), WPE_INPUT_SOURCE_MOUSE, time, mods,
                    number(o, "dx"), number(o, "dy"), json_object_get_boolean_member(o, "precise"),
                    json_object_get_boolean_member(o, "stop"), x, y));
        else if (!strcmp(cmd, "down") || !strcmp(cmd, "up")) {
            gboolean down = !strcmp(cmd, "down");
            guint button = number(o, "button");
            if (button >= 1 && button <= 5) {
                WPEModifiers flag = WPE_MODIFIER_POINTER_BUTTON1 << (button - 1);
                if (down)
                    page->pressed |= flag;
                else
                    page->pressed &= ~flag;
                mods = modifiers(number(o, "mods")) | page->pressed;
            }
            event(page,
                wpe_event_pointer_button_new(down ? WPE_EVENT_POINTER_DOWN : WPE_EVENT_POINTER_UP,
                    WPE_VIEW(page->view), WPE_INPUT_SOURCE_MOUSE, time, mods, button, x, y,
                    down ? number(o, "count") : 0));
        } else {
            WPEEventType type = !strcmp(cmd, "enter") ? WPE_EVENT_POINTER_ENTER
                : !strcmp(cmd, "leave")               ? WPE_EVENT_POINTER_LEAVE
                                                      : WPE_EVENT_POINTER_MOVE;
            event(page,
                wpe_event_pointer_move_new(type, WPE_VIEW(page->view), WPE_INPUT_SOURCE_MOUSE, time,
                    mods, x, y, x - page->pointer_x, y - page->pointer_y));
        }
        page->pointer_x = x;
        page->pointer_y = y;
    } else if (!strcmp(cmd, "dismiss-menu"))
        dismiss(page);
    else if (g_str_has_prefix(cmd, "menu-")) {
        guint index = g_ascii_strtoull(cmd + 5, NULL, 10);
        if (index < page->actions->len) {
            WebKitContextMenuItem* item = g_ptr_array_index(page->actions, index);
            g_action_activate(webkit_context_menu_item_get_gaction(item),
                webkit_context_menu_item_get_gaction_target(item));
        }
        dismiss(page);
    } else if (g_str_has_prefix(cmd, "option-")) {
        guint index = g_ascii_strtoull(cmd + 7, NULL, 10);
        if (page->options && index < webkit_option_menu_get_n_items(page->options))
            webkit_option_menu_activate_item(page->options, index);
        dismiss(page);
    } else {
        const char* edit = !strcmp(cmd, "copy") ? WEBKIT_EDITING_COMMAND_COPY
            : !strcmp(cmd, "cut")               ? WEBKIT_EDITING_COMMAND_CUT
            : !strcmp(cmd, "paste")             ? WEBKIT_EDITING_COMMAND_PASTE
            : !strcmp(cmd, "select-all")        ? WEBKIT_EDITING_COMMAND_SELECT_ALL
            : !strcmp(cmd, "undo")              ? WEBKIT_EDITING_COMMAND_UNDO
            : !strcmp(cmd, "redo")              ? WEBKIT_EDITING_COMMAND_REDO
                                                : NULL;
        if (edit)
            webkit_web_view_execute_editing_command(page->web, edit);
        else
            failure(id, "Unknown browser command");
    }
}
static gboolean readable(gint fd, GIOCondition condition, gpointer data)
{
    guint8 bytes[16384];
    ssize_t count = recv(fd, bytes, sizeof bytes, MSG_DONTWAIT);
    if (count < 0 && (errno == EINTR || errno == EAGAIN))
        return G_SOURCE_CONTINUE;
    if (count <= 0) {
        g_main_loop_quit(loop);
        return G_SOURCE_REMOVE;
    }
    g_byte_array_append(incoming, bytes, count);
    while (incoming->len >= 4) {
        guint32 length;
        memcpy(&length, incoming->data, 4);
        length = GUINT32_FROM_LE(length);
        if (length > PACKET_LIMIT) {
            failure(0, "Browser command is too large");
            g_main_loop_quit(loop);
            return G_SOURCE_REMOVE;
        }
        if (incoming->len < length + 4)
            break;
        JsonParser* parser = json_parser_new();
        GError* error = NULL;
        if (!json_parser_load_from_data(parser, (char*)incoming->data + 4, length, &error)
            || !JSON_NODE_HOLDS_OBJECT(json_parser_get_root(parser))) {
            failure(0, error ? error->message : "Invalid browser command");
            g_clear_error(&error);
            g_object_unref(parser);
            g_main_loop_quit(loop);
            return G_SOURCE_REMOVE;
        }
        command(json_node_get_object(json_parser_get_root(parser)));
        g_object_unref(parser);
        g_byte_array_remove_range(incoming, 0, length + 4);
    }
    return G_SOURCE_CONTINUE;
}
int main(int argc, char** argv)
{
    const char *profile = NULL, *node = NULL, *modifiers_list = NULL;
    for (int i = 1; i + 1 < argc; i += 2) {
        if (!strcmp(argv[i], "--profile"))
            profile = argv[i + 1];
        else if (!strcmp(argv[i], "--render-node"))
            node = argv[i + 1];
        else if (!strcmp(argv[i], "--modifiers"))
            modifiers_list = argv[i + 1];
        else
            return 2;
    }
    int profile_handle = -1;
    if (profile) {
        char* path = g_build_filename(profile, "profile.lock", NULL);
        profile_handle = open(path, O_CREAT | O_RDWR | O_CLOEXEC, 0600);
        g_free(path);
        if (profile_handle < 0 || flock(profile_handle, LOCK_EX | LOCK_NB) < 0) {
            failure(0, "Browser profile cannot be locked; close other users of this profile");
            if (profile_handle >= 0)
                close(profile_handle);
            return 1;
        }
    }
    loop = g_main_loop_new(NULL, FALSE);
    pages = g_hash_table_new_full(g_direct_hash, g_direct_equal, NULL, page_free);
    incoming = g_byte_array_new();
    display = g_object_new(bezel_display_get_type(), NULL);
    if (node) {
        display->device = wpe_drm_device_new(NULL, node);
        WPEBufferFormatsBuilder* builder = wpe_buffer_formats_builder_new(display->device);
        wpe_buffer_formats_builder_append_group(
            builder, display->device, WPE_BUFFER_FORMAT_USAGE_RENDERING);
        if (modifiers_list && *modifiers_list) {
            char** values = g_strsplit(modifiers_list, ",", -1);
            for (guint i = 0; values[i]; i++)
                wpe_buffer_formats_builder_append_format(
                    builder, ARGB8888, g_ascii_strtoull(values[i], NULL, 16));
            g_strfreev(values);
        } else
            wpe_buffer_formats_builder_append_format(builder, ARGB8888, 0);
        display->formats = wpe_buffer_formats_builder_end(builder);
        wpe_buffer_formats_builder_unref(builder);
    }
    GError* error = NULL;
    if (!wpe_display_connect(WPE_DISPLAY(display), &error)) {
        failure(0, error->message);
        return 1;
    }
    if (profile) {
        char* cache = g_build_filename(profile, "cache", NULL);
        network = webkit_network_session_new(profile, cache);
        g_free(cache);
        char* cookies = g_build_filename(profile, "cookies.sqlite", NULL);
        webkit_cookie_manager_set_persistent_storage(
            webkit_network_session_get_cookie_manager(network), cookies,
            WEBKIT_COOKIE_PERSISTENT_STORAGE_SQLITE);
        g_free(cookies);
    } else
        network = webkit_network_session_new_ephemeral();
    JsonObject* ready = object();
    char* version = g_strdup_printf("%u.%u.%u", webkit_get_major_version(),
        webkit_get_minor_version(), webkit_get_micro_version());
    string(ready, "webkit", version);
    g_free(version);
    json_packet('R', 0, ready, -1);
    g_unix_fd_add(0, G_IO_IN | G_IO_HUP | G_IO_ERR, readable, NULL);
    g_main_loop_run(loop);
    g_hash_table_unref(pages);
    g_object_unref(network);
    g_object_unref(display);
    g_byte_array_unref(incoming);
    g_main_loop_unref(loop);
    if (profile_handle >= 0)
        close(profile_handle);
    return 0;
}
