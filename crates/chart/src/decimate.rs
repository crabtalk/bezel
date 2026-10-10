//! Thinning a line to what its pixels can show.

/// M4: of each run of points in one pixel column, the first, the lowest, the
/// highest and the last, in their order. A line drawn through what is kept
/// covers the same pixels as one drawn through every point. `points` run in
/// ascending x.
pub fn m4(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut kept = Vec::new();
    let mut start = 0;
    while start < points.len() {
        let column = points[start][0].floor();
        let (mut low, mut high, mut end) = (start, start, start);
        while end < points.len() && points[end][0].floor() == column {
            if points[end][1] < points[low][1] {
                low = end;
            }
            if points[end][1] > points[high][1] {
                high = end;
            }
            end += 1;
        }
        let mut picks = [start, low, high, end - 1];
        picks.sort_unstable();
        for (index, &pick) in picks.iter().enumerate() {
            if index == 0 || pick != picks[index - 1] {
                kept.push(points[pick]);
            }
        }
        start = end;
    }
    kept
}
