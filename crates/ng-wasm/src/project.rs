// Project rendering utilities
use ng_core::EncodeMeta;
use ng_export::wav::{WavDepth, WavEncoder, WavOptions};
use ng_render::{OfflineOptions, render_offline};
use ng_score::{migrate, validate};
use ng_synth::osc::OscSynth;
use ng_timeline::{compile, TempoMap};

pub(crate) fn render_project(
    bytes: &[u8],
    sample_rate: u32,
    depth: WavDepth,
) -> Result<Vec<u8>, (&'static str, String)> {
    let project = migrate(bytes).map_err(|e| (e.code, e.message))?;
    let report = validate(&project);
    if !report.errors.is_empty() {
        return Err((
            "E_FORMAT",
            report
                .errors
                .iter()
                .map(|e| format!("{} at {}", e.code, e.path))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    let schedule = compile(&project, sample_rate).map_err(|e| ("E_FORMAT", format!("{e}")))?;
    let loop_points = match (
        TempoMap::from_project(&project, sample_rate),
        project.loop_region,
    ) {
        (Ok(tm), Some(region)) => {
            let start = tm.tick_to_sample(region.start) as u32;
            let end = tm.tick_to_sample(region.end) as u32;
            Some((start, end))
        }
        _ => None,
    };
    let synth = OscSynth::new(sample_rate, 64);
    let opts = OfflineOptions::default();
    let pcm = render_offline(schedule, synth, &opts, |_| true)
        .map_err(|e| ("E_RENDER", format!("{e:?}")))?;
    let meta = EncodeMeta {
        title: Some(project.title.clone()),
        loop_points,
    };
    let enc = WavEncoder::new();
    ng_core::AudioEncoder::encode(
        &enc,
        &pcm,
        &meta,
        &WavOptions {
            depth,
            ..Default::default()
        },
    )
    .map_err(|e| (e.code, e.message))
}
