use gallery::{DIAGRAMS, pages::diagrams::mermaid::SAMPLES};

/// Every diagram row has a sample, and every sample a row.
#[test]
fn rows_and_samples_agree() {
    let rows: Vec<_> = DIAGRAMS
        .iter()
        .flat_map(|group| group.sections)
        .map(|section| section.key)
        .filter(|key| key.starts_with("mermaid-"))
        .collect();
    let samples: Vec<_> = SAMPLES.iter().map(|sample| sample.key).collect();
    assert_eq!(
        rows, samples,
        "the mermaid rows and their samples, in order"
    );
}

/// A sample that fails to draw shows as code with an error under it.
#[test]
fn every_sample_draws() {
    for sample in SAMPLES {
        if let Err(error) = mermaid::render(sample.source, &Default::default(), &mermaid::Metrics) {
            panic!("{} does not draw: {error:?}", sample.key);
        }
    }
}
