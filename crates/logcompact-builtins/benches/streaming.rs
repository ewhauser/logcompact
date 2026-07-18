use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use logcompact_builtins::{
    Budget, BuiltinParserOptions, EndReason, GenericRanker, NoPathMapping, NoRedaction,
    OutputPolicy, ParserPlan, ProblemMatcherRegistry, ReductionOptions, ReductionSession, Scope,
    SessionOptions, Stream, TextInput, builtin_parser_plan, reduce,
};

fn corpus(lines: usize, match_every: Option<usize>, repeated: bool) -> Vec<u8> {
    let mut output = String::new();
    for index in 0..lines {
        if match_every.is_some_and(|interval| index % interval == interval - 1) {
            if repeated {
                output.push_str("src/repeated.cc:42:7: error: missing repeated value\n");
            } else {
                output.push_str(&format!(
                    "src/file{index}.cc:{}:7: error: missing value {index}\n",
                    index + 1
                ));
            }
        } else {
            output.push_str(&format!("[{index}] ordinary command progress output\n"));
        }
    }
    output.into_bytes()
}

fn stream(input: &[u8], chunk_size: usize) {
    let mut session = ReductionSession::new(
        builtin_parser_plan(BuiltinParserOptions::default()),
        SessionOptions {
            budget: Budget {
                max_bytes: 64 * 1024,
                max_items: 100,
            },
            ..SessionOptions::default()
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

#[derive(Clone, Copy)]
enum ParserMode {
    Builtins,
    Matcher,
    Combined,
}

fn problem_matcher_registry() -> ProblemMatcherRegistry {
    let mut registry = ProblemMatcherRegistry::default();
    registry
        .add_json(
            br#"{
            "problemMatcher": [{
                "owner": "benchmark",
                "pattern": {
                    "regexp": "^MATCH (.+):(\\d+):(\\d+) \\[(error|warning)\\] (.+)$",
                    "file": 1,
                    "line": 2,
                    "column": 3,
                    "severity": 4,
                    "message": 5
                }
            }]
        }"#,
        )
        .expect("the benchmark matcher should compile");
    registry
}

fn problem_matcher_corpus(lines: usize, match_every: Option<usize>) -> Vec<u8> {
    let mut output = String::new();
    for index in 0..lines {
        if match_every.is_some_and(|interval| index % interval == interval - 1) {
            output.push_str(&format!(
                "MATCH src/file{index}.dsl:{}:7 [error] invalid widget {index}\n",
                index + 1
            ));
        } else {
            output.push_str(&format!("[{index}] ordinary command progress output\n"));
        }
    }
    output.into_bytes()
}

fn stream_problem_matcher(
    input: &[u8],
    chunk_size: usize,
    mode: ParserMode,
    registry: &ProblemMatcherRegistry,
) {
    let mut plan = match mode {
        ParserMode::Builtins | ParserMode::Combined => {
            builtin_parser_plan(BuiltinParserOptions::default())
        }
        ParserMode::Matcher => ParserPlan::new(),
    };
    if matches!(mode, ParserMode::Matcher | ParserMode::Combined) {
        plan.push(registry.clone().into_parser())
            .expect("the problem matcher parser identifier should be unique");
    }
    let mut session = ReductionSession::new(
        plan,
        SessionOptions {
            budget: Budget {
                max_bytes: 64 * 1024,
                max_items: 100,
            },
            ..SessionOptions::default()
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

fn generic_reducers(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("generic_reducers");
    for (name, input) in [
        ("no-match-tail", corpus(25_000, None, false)),
        ("mixed-tail", corpus(25_000, Some(1_000), false)),
        ("match-heavy", corpus(25_000, Some(2), false)),
        ("repeated-diagnostic", corpus(25_000, Some(2), true)),
    ] {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_function(format!("batch/{name}"), |bencher| {
            bencher.iter(|| {
                black_box(reduce(
                    &[TextInput::new(black_box(&input))],
                    &ReductionOptions {
                        budget: Budget {
                            max_bytes: 64 * 1024,
                            max_items: 100,
                        },
                        ..ReductionOptions::default()
                    },
                    &NoRedaction,
                ));
            });
        });
        group.bench_function(format!("stream-64k/{name}"), |bencher| {
            bencher.iter(|| stream(&input, 64 * 1024));
        });
        group.bench_function(format!("stream-1k/{name}"), |bencher| {
            bencher.iter(|| stream(&input, 1024));
        });
        group.bench_function(format!("stream-7b/{name}"), |bencher| {
            bencher.iter(|| stream(&input, 7));
        });
    }
    group.finish();
}

fn problem_matcher_modes(criterion: &mut Criterion) {
    let registry = problem_matcher_registry();
    let mut group = criterion.benchmark_group("problem_matchers");
    for (input_name, input) in [
        ("no-match", problem_matcher_corpus(25_000, None)),
        ("sparse-match", problem_matcher_corpus(25_000, Some(1_000))),
    ] {
        group.throughput(Throughput::Bytes(input.len() as u64));
        for (mode_name, mode) in [
            ("builtins-only", ParserMode::Builtins),
            ("matcher-only", ParserMode::Matcher),
            ("combined", ParserMode::Combined),
        ] {
            group.bench_function(format!("{input_name}/{mode_name}"), |bencher| {
                bencher.iter(|| {
                    stream_problem_matcher(&input, 64 * 1024, mode, &registry);
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, generic_reducers, problem_matcher_modes);
criterion_main!(benches);
