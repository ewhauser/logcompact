use std::fmt::Write as _;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use logcompact_core::{
    Budget, EndReason, GenericRanker, Limits, NoPathMapping, NoRedaction, OutputPolicy, ParserPlan,
    ReductionSession, Scope, SessionOptions, Stream, deduplicate_lines,
};

fn deduplication_corpus(lines: usize, unique: bool) -> String {
    let mut output = String::new();
    for index in 0..lines {
        if unique {
            writeln!(output, "warning: unique diagnostic {index}")
                .expect("writing to a string cannot fail");
        } else {
            output.push_str("warning: repeated\n");
        }
    }
    output
}

fn deduplication(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("core/deduplication");
    for lines in [100_usize, 10_000, 100_000] {
        for unique in [false, true] {
            let kind = if unique { "unique" } else { "repeated" };
            let input = deduplication_corpus(lines, unique);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(kind, lines), &input, |bencher, input| {
                bencher.iter(|| deduplicate_lines(black_box(input)));
            });
        }
    }
    group.finish();
}

fn framing_corpus(lines: usize, separator: &str) -> Vec<u8> {
    let mut output = String::new();
    for index in 0..lines {
        write!(output, "[{index}] ordinary progress output{separator}")
            .expect("writing to a string cannot fail");
    }
    output.into_bytes()
}

fn frame(input: &[u8], chunk_size: usize) {
    let mut session = ReductionSession::new(
        ParserPlan::new(),
        SessionOptions {
            budget: Budget {
                max_bytes: 64 * 1024,
                max_items: 100,
            },
            limits: Limits {
                max_scope_bytes: input.len(),
                ..Limits::default()
            },
        },
        OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
    );
    session.begin_scope(Scope::step("benchmark"));
    for chunk in input.chunks(chunk_size) {
        session.push_chunk("benchmark", Stream::Stderr, black_box(chunk));
    }
    session.end_scope("benchmark", EndReason::Complete);
    black_box(session.finish());
}

fn framing(criterion: &mut Criterion) {
    const LINE_COUNT: usize = 25_000;

    let mut group = criterion.benchmark_group("core/framing");
    for (line_ending, separator) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let input = framing_corpus(LINE_COUNT, separator);
        group.throughput(Throughput::Bytes(input.len() as u64));
        for (chunk_name, chunk_size) in [("7b", 7), ("1k", 1024), ("64k", 64 * 1024)] {
            group.bench_with_input(
                BenchmarkId::new(format!("{chunk_name}/{line_ending}"), LINE_COUNT),
                &input,
                |bencher, input| bencher.iter(|| frame(input, chunk_size)),
            );
        }
    }
    group.finish();
}

criterion_group!(benches, deduplication, framing);
criterion_main!(benches);
