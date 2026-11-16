#include <wpe/webkit.h>
#include <wpe/wpe-platform.h>
#include <wpe/headless/wpe-headless.h>
#include <glib-unix.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    WPEView parent;
    GHashTable *buffers;
    guint sequence;
} ProofView;
typedef struct { WPEViewClass parent; } ProofViewClass;
typedef struct { WPEDisplay parent; } ProofDisplay;
typedef struct { WPEDisplayClass parent; } ProofDisplayClass;
G_DEFINE_TYPE(ProofView, proof_view, WPE_TYPE_VIEW)
G_DEFINE_TYPE(ProofDisplay, proof_display, WPE_TYPE_DISPLAY)

static GMainLoop *loop;
static ProofView *view;

static gboolean render(WPEView *base, WPEBuffer *buffer,
                       const WPERectangle *damage, guint count, GError **error) {
    if (!WPE_IS_BUFFER_SHM(buffer)) {
        g_set_error_literal(error, WPE_BUFFER_ERROR, WPE_BUFFER_ERROR_NOT_SUPPORTED,
                            "Proof requires a shared-memory frame");
        return FALSE;
    }
    GBytes *pixels = wpe_buffer_import_to_pixels(buffer, error);
    if (!pixels)
        return FALSE;
    gsize length;
    const void *bytes = g_bytes_get_data(pixels, &length);
    ProofView *self = (ProofView *)base;
    guint id = ++self->sequence;
    g_hash_table_insert(self->buffers, GUINT_TO_POINTER(id), g_object_ref(buffer));
    printf("F %u %d %d %zu\n", id, wpe_buffer_get_width(buffer),
           wpe_buffer_get_height(buffer), length);
    if (fwrite(bytes, 1, length, stdout) != length || fflush(stdout))
        exit(2);
    return TRUE;
}

static void attached(WPEView *base, GParamSpec *property, gpointer data) {
    if (wpe_view_get_toplevel(base)) {
        wpe_view_resized(base, 320, 200);
        wpe_view_map(base);
    } else {
        wpe_view_unmap(base);
    }
}

static void dispose(GObject *object) {
    ProofView *self = (ProofView *)object;
    g_clear_pointer(&self->buffers, g_hash_table_unref);
    G_OBJECT_CLASS(proof_view_parent_class)->dispose(object);
}

static void proof_view_init(ProofView *self) {
    self->buffers = g_hash_table_new_full(g_direct_hash, g_direct_equal,
                                        NULL, g_object_unref);
    g_signal_connect(self, "notify::toplevel", G_CALLBACK(attached), NULL);
}

static void proof_view_class_init(ProofViewClass *klass) {
    WPE_VIEW_CLASS(klass)->render_buffer = render;
    G_OBJECT_CLASS(klass)->dispose = dispose;
}

static gboolean connect_display(WPEDisplay *display, GError **error) {
    return TRUE;
}

static WPEView *create_view(WPEDisplay *display) {
    return g_object_new(proof_view_get_type(), "display", display, NULL);
}

static WPEToplevel *create_toplevel(WPEDisplay *display, guint max_views) {
    return g_object_new(WPE_TYPE_TOPLEVEL_HEADLESS, "display", display,
                        "max-views", max_views, NULL);
}

static void proof_display_init(ProofDisplay *display) { }

static void proof_display_class_init(ProofDisplayClass *klass) {
    WPEDisplayClass *display = WPE_DISPLAY_CLASS(klass);
    display->connect = connect_display;
    display->create_view = create_view;
    display->create_toplevel = create_toplevel;
}

static void send_event(WPEEvent *event) {
    wpe_view_event(WPE_VIEW(view), event);
    wpe_event_unref(event);
}

static gboolean command(GIOChannel *channel, GIOCondition condition, gpointer data) {
    gchar *line = NULL;
    if (g_io_channel_read_line(channel, &line, NULL, NULL, NULL) != G_IO_STATUS_NORMAL) {
        g_main_loop_quit(loop);
        return G_SOURCE_REMOVE;
    }
    guint id, code, value;
    double x, y;
    if (sscanf(line, "release %u", &id) == 1) {
        WPEBuffer *buffer = g_hash_table_lookup(view->buffers, GUINT_TO_POINTER(id));
        if (!buffer)
            g_error("Unknown buffer %u", id);
        wpe_view_buffer_rendered(WPE_VIEW(view), buffer);
        wpe_view_buffer_released(WPE_VIEW(view), buffer);
        g_hash_table_remove(view->buffers, GUINT_TO_POINTER(id));
        printf("R %u\n", id);
    } else if (sscanf(line, "click %lf %lf", &x, &y) == 2) {
        guint32 time = g_get_monotonic_time() / 1000;
        wpe_view_focus_in(WPE_VIEW(view));
        send_event(wpe_event_pointer_move_new(
            WPE_EVENT_POINTER_ENTER, WPE_VIEW(view), WPE_INPUT_SOURCE_MOUSE,
            time, 0, x, y, 0, 0));
        send_event(wpe_event_pointer_move_new(
            WPE_EVENT_POINTER_MOVE, WPE_VIEW(view), WPE_INPUT_SOURCE_MOUSE,
            time, 0, x, y, 0, 0));
        send_event(wpe_event_pointer_button_new(
            WPE_EVENT_POINTER_DOWN, WPE_VIEW(view), WPE_INPUT_SOURCE_MOUSE,
            time, WPE_MODIFIER_POINTER_BUTTON1, WPE_BUTTON_PRIMARY, x, y, 1));
        send_event(wpe_event_pointer_button_new(
            WPE_EVENT_POINTER_UP, WPE_VIEW(view), WPE_INPUT_SOURCE_MOUSE,
            time + 1, 0, WPE_BUTTON_PRIMARY, x, y, 0));
    } else if (sscanf(line, "key %u %u", &code, &value) == 2) {
        guint32 time = g_get_monotonic_time() / 1000;
        send_event(wpe_event_keyboard_new(
            WPE_EVENT_KEYBOARD_KEY_DOWN, WPE_VIEW(view), WPE_INPUT_SOURCE_KEYBOARD,
            time, 0, code, value));
        send_event(wpe_event_keyboard_new(
            WPE_EVENT_KEYBOARD_KEY_UP, WPE_VIEW(view), WPE_INPUT_SOURCE_KEYBOARD,
            time + 1, 0, code, value));
    } else if (g_str_has_prefix(line, "held")) {
        printf("H %u\n", g_hash_table_size(view->buffers));
    } else if (g_str_has_prefix(line, "quit")) {
        g_main_loop_quit(loop);
    } else {
        g_error("Unknown command");
    }
    fflush(stdout);
    g_free(line);
    return G_SOURCE_CONTINUE;
}

static void title(WebKitWebView *web, GParamSpec *property, gpointer data) {
    printf("S %s\n", webkit_web_view_get_title(web));
    fflush(stdout);
}

static gboolean timeout(gpointer data) {
    g_error("Proof timed out");
    return G_SOURCE_REMOVE;
}

int main(void) {
    GError *error = NULL;
    WPEDisplay *display = g_object_new(proof_display_get_type(), NULL);
    if (!wpe_display_connect(display, &error))
        g_error("%s", error->message);
    WebKitWebView *web = g_object_new(WEBKIT_TYPE_WEB_VIEW, "display", display, NULL);
    view = (ProofView *)webkit_web_view_get_wpe_view(web);
    WPEToplevel *top = wpe_display_create_toplevel(display, 1);
    wpe_view_set_toplevel(WPE_VIEW(view), top);
    wpe_toplevel_resize(top, 320, 200);
    wpe_toplevel_state_changed(top, WPE_TOPLEVEL_STATE_ACTIVE);
    g_signal_connect(web, "notify::title", G_CALLBACK(title), NULL);
    loop = g_main_loop_new(NULL, FALSE);
    GIOChannel *input = g_io_channel_unix_new(0);
    g_io_add_watch(input, G_IO_IN | G_IO_HUP, command, NULL);
    webkit_web_view_load_html(web,
        "<!doctype html><meta name='viewport' content='width=device-width,initial-scale=1'>"
        "<style>html{background:#e02030}body{margin:0}button,input{position:absolute;"
        "left:20px;width:100px;height:40px}button{top:20px}input{top:80px}</style>"
        "<button onclick='++clicks;document.documentElement.style.background=\"#20e030\";"
        "report()'>Click</button>"
        "<input onfocus='report()' oninput='document.documentElement.style.background="
        "\"#2030e0\";report()'>"
        "<script>let clicks=0;function report(){document.title=[innerWidth,innerHeight,clicks,"
        "document.querySelector('input').value,document.activeElement.tagName==='INPUT'?1:0]"
        ".join(':')}onload=report;onresize=report</script>", "about:blank");
    g_timeout_add_seconds(60, timeout, NULL);
    g_main_loop_run(loop);
    wpe_view_set_toplevel(WPE_VIEW(view), NULL);
    g_object_unref(web);
    g_object_unref(top);
    g_object_unref(display);
    g_io_channel_unref(input);
    g_main_loop_unref(loop);
    return 0;
}
