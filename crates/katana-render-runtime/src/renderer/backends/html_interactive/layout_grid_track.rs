use super::style::{CssGridTrack, CssGridTrackBreadth};
use taffy::style::{MaxTrackSizingFunction, MinTrackSizingFunction};
use taffy::style_helpers::{auto, fr, length, max_content, min_content, minmax, percent};

pub(super) fn taffy_grid_track(track: CssGridTrack) -> taffy::style::TrackSizingFunction {
    match track {
        CssGridTrack::Length(value) => length(value),
        CssGridTrack::Percent(value) => percent(value),
        CssGridTrack::Fraction(value) => fr(value),
        CssGridTrack::Auto => auto(),
        CssGridTrack::MinContent => min_content(),
        CssGridTrack::MaxContent => max_content(),
        CssGridTrack::MinMax { min, max } => minmax(taffy_min_track(min), taffy_max_track(max)),
    }
}

fn taffy_min_track(track: CssGridTrackBreadth) -> MinTrackSizingFunction {
    match track {
        CssGridTrackBreadth::Length(value) => length(value),
        CssGridTrackBreadth::Percent(value) => percent(value),
        CssGridTrackBreadth::Auto => auto(),
        CssGridTrackBreadth::MinContent => min_content(),
        CssGridTrackBreadth::MaxContent => max_content(),
        CssGridTrackBreadth::Fraction(_) => unreachable!("CSS min track cannot use fr"),
    }
}

fn taffy_max_track(track: CssGridTrackBreadth) -> MaxTrackSizingFunction {
    match track {
        CssGridTrackBreadth::Length(value) => length(value),
        CssGridTrackBreadth::Percent(value) => percent(value),
        CssGridTrackBreadth::Fraction(value) => fr(value),
        CssGridTrackBreadth::Auto => auto(),
        CssGridTrackBreadth::MinContent => min_content(),
        CssGridTrackBreadth::MaxContent => max_content(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CssGridTrack, CssGridTrackBreadth, taffy_grid_track, taffy_max_track, taffy_min_track,
    };

    #[test]
    fn grid_track_conversion_preserves_intrinsic_and_percent_sizing() {
        let percent = taffy_grid_track(CssGridTrack::Percent(0.25));
        assert_eq!(
            percent
                .max_sizing_function()
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(50.0)
        );
        assert!(
            taffy_grid_track(CssGridTrack::Auto)
                .max_sizing_function()
                .is_auto()
        );
        assert!(
            taffy_grid_track(CssGridTrack::MinContent)
                .max_sizing_function()
                .is_min_content()
        );
        assert!(
            taffy_grid_track(CssGridTrack::MaxContent)
                .max_sizing_function()
                .is_max_content()
        );
    }

    #[test]
    fn grid_track_conversion_preserves_minmax_sizing() {
        let minmax = taffy_grid_track(CssGridTrack::MinMax {
            min: CssGridTrackBreadth::Length(80.0),
            max: CssGridTrackBreadth::Fraction(1.0),
        });
        assert_eq!(
            minmax
                .min_sizing_function()
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(80.0)
        );
        assert!(minmax.max_sizing_function().is_fr());
    }

    #[test]
    fn minimum_track_conversion_covers_each_supported_breadth() {
        assert_eq!(
            taffy_min_track(CssGridTrackBreadth::Percent(0.25))
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(50.0)
        );
        assert!(taffy_min_track(CssGridTrackBreadth::Auto).is_auto());
        assert!(taffy_min_track(CssGridTrackBreadth::MinContent).is_min_content());
        assert!(taffy_min_track(CssGridTrackBreadth::MaxContent).is_max_content());
    }

    #[test]
    fn maximum_track_conversion_covers_each_supported_breadth() {
        assert_eq!(
            taffy_max_track(CssGridTrackBreadth::Length(40.0))
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(40.0)
        );
        assert_eq!(
            taffy_max_track(CssGridTrackBreadth::Percent(0.25))
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(50.0)
        );
        assert!(taffy_max_track(CssGridTrackBreadth::Auto).is_auto());
        assert!(taffy_max_track(CssGridTrackBreadth::MinContent).is_min_content());
        assert!(taffy_max_track(CssGridTrackBreadth::MaxContent).is_max_content());
    }

    #[test]
    #[should_panic(expected = "CSS min track cannot use fr")]
    fn minimum_track_rejects_fraction_breadth() {
        let _ = taffy_min_track(CssGridTrackBreadth::Fraction(1.0));
    }
}
