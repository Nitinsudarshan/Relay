//! A throwaway that answers one question: does `ort` build, bundle and run
//! inside a real Relay binary on Windows?
//!
//! Two approved features are waiting on that answer — G6 denoise and T4 smart
//! turn detection ([`maybe_later.md`] items 1 and 3) — and neither is worth
//! starting before it is known. The risk is not the inference code; it is the
//! packaging. `ort` links a C++ runtime that Tauri's bundler has never been
//! asked to carry, and the failure everyone hits is not a compile error, it is
//! an installer that produces an executable which cannot find
//! `onnxruntime.dll` on a machine that is not the developer's.
//!
//! So the spike is deliberately shaped like the real thing rather than like a
//! unit test: it is compiled into the Tauri binary, behind the `onnx-spike`
//! feature, and it runs on launch. Running the *installed* app is the
//! measurement. See `docs/spikes/onnx-windows.md` for the procedure and for
//! what each outcome means.
//!
//! Nothing here has been run. It was written on Linux and cannot be validated
//! from that container — proving that is the point of handing it over.
//!
//! **The model is built here, not shipped.** Relay bundles no model files
//! (`README.md`), and a spike is a poor reason to be the first. [`tiny_add_model`]
//! emits a valid ONNX graph — `c = a + b` over three floats — as bytes, so the
//! thing being proven stays "ONNX Runtime executed a graph", with no download,
//! no fixture in git, and no second question about where model files live.
//! That question is real, and it is the product decision that comes *after*
//! this spike passes, not part of it.

use serde::{Deserialize, Serialize};

/// The name Relay writes its spike report under, in the OS temp directory.
///
/// A file rather than only a log line because on Windows a bundled Tauri app is
/// a GUI subsystem binary: it has no console, and `tracing` output goes
/// nowhere the person running the installer will see.
pub const REPORT_FILE: &str = "relay-onnx-spike.json";

/// What one run of the spike found.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpikeReport {
    /// Whether this binary was built with `--features onnx-spike` at all.
    ///
    /// A report with this `false` is the answer to "did I remember the
    /// feature flag", which is the first thing that goes wrong.
    pub ort_compiled_in: bool,
    /// The directory the running executable was launched from, and whether an
    /// `onnxruntime` dynamic library is sitting next to it.
    ///
    /// This is the packaging question in one line. `ort`'s `copy-dylibs`
    /// feature puts the library beside the binary in `target/`; whether
    /// Tauri's bundler then carries it into the installer is exactly what
    /// nobody can answer from a dev machine.
    pub exe_dir: Option<String>,
    pub dylib_beside_exe: Vec<String>,
    /// Size of the generated model, as a cheap check that the builder ran.
    pub model_bytes: usize,
    /// Present when inference was attempted, whatever the result.
    pub inference: Option<InferenceOutcome>,
    /// Whatever went wrong, in the words of whatever went wrong.
    pub error: Option<String>,
}

/// The result of running `c = a + b` once.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferenceOutcome {
    pub expected: Vec<f32>,
    pub actual: Vec<f32>,
    /// The only line that matters. `true` means ONNX Runtime loaded, built a
    /// session from bytes, and computed the right answer inside this binary.
    pub correct: bool,
    pub elapsed_ms: u128,
}

/// The inputs, fixed so the expected output is fixed.
///
/// Public because they are the reading key for the report: a run that comes
/// back with `[11, 22, 33]` executed the graph, and anything else did not.
pub const INPUT_A: [f32; 3] = [1.0, 2.0, 3.0];
pub const INPUT_B: [f32; 3] = [10.0, 20.0, 30.0];
pub const EXPECTED: [f32; 3] = [11.0, 22.0, 33.0];

/// Runs the spike and writes its report where a person can find it.
///
/// Never panics and never propagates: a spike that takes the app down with it
/// tells you less than one that writes down what happened.
pub fn run_and_record() {
    let report = run();

    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            let path = std::env::temp_dir().join(REPORT_FILE);
            match std::fs::write(&path, &json) {
                Ok(()) => tracing::info!("[OnnxSpike] Report written to {}", path.display()),
                Err(e) => tracing::warn!("[OnnxSpike] Could not write {}: {e}", path.display()),
            }
            tracing::info!("[OnnxSpike] {json}");
        }
        Err(e) => tracing::warn!("[OnnxSpike] Could not serialize the report: {e}"),
    }
}

/// Runs the spike and returns its report.
pub fn run() -> SpikeReport {
    let model = tiny_add_model();
    let (exe_dir, dylib_beside_exe) = neighbouring_runtime_libraries();

    let mut report = SpikeReport {
        ort_compiled_in: cfg!(feature = "onnx-spike"),
        exe_dir,
        dylib_beside_exe,
        model_bytes: model.len(),
        inference: None,
        error: None,
    };

    #[cfg(feature = "onnx-spike")]
    match infer(&model) {
        Ok(outcome) => report.inference = Some(outcome),
        Err(e) => report.error = Some(e),
    }

    #[cfg(not(feature = "onnx-spike"))]
    {
        let _ = &model;
        report.error = Some(
            "built without --features onnx-spike; the model was generated but nothing ran it"
                .to_string(),
        );
    }

    report
}

/// Whatever ONNX Runtime library is sitting next to the executable.
///
/// Reported rather than required: a statically linked build has none and is
/// the *better* outcome. What would be bad is a `target/` build that finds one
/// and an installed build that does not — which is the failure this line is
/// here to make visible.
fn neighbouring_runtime_libraries() -> (Option<String>, Vec<String>) {
    let Ok(exe) = std::env::current_exe() else {
        return (None, Vec::new());
    };
    let Some(dir) = exe.parent() else {
        return (None, Vec::new());
    };

    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.to_ascii_lowercase()
                .contains("onnxruntime")
                .then_some(name)
        })
        .collect();
    found.sort();

    (Some(dir.display().to_string()), found)
}

#[cfg(feature = "onnx-spike")]
fn infer(model: &[u8]) -> Result<InferenceOutcome, String> {
    use ort::session::Session;
    use ort::value::Tensor;

    let started = std::time::Instant::now();

    let mut session = Session::builder()
        .map_err(|e| format!("Session::builder: {e}"))?
        .commit_from_memory(model)
        .map_err(|e| format!("commit_from_memory: {e}"))?;

    let a = Tensor::from_array(([INPUT_A.len()], INPUT_A.to_vec()))
        .map_err(|e| format!("input a: {e}"))?;
    let b = Tensor::from_array(([INPUT_B.len()], INPUT_B.to_vec()))
        .map_err(|e| format!("input b: {e}"))?;

    let outputs = session
        .run(ort::inputs!["a" => a, "b" => b])
        .map_err(|e| format!("run: {e}"))?;

    let value = outputs.get("c").ok_or_else(|| "no output named c".to_string())?;
    let (_shape, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("extract: {e}"))?;

    let actual = data.to_vec();
    Ok(InferenceOutcome {
        expected: EXPECTED.to_vec(),
        correct: actual == EXPECTED,
        actual,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

// ---------------------------------------------------------------------------
// The model, as bytes.
//
// ONNX is protobuf, and the graph being proven is three fields deep, so the
// encoder below is the whole dependency: a varint writer and a
// length-delimited writer. Adding `prost` and the ONNX schema to generate one
// `Add` node would be a larger commitment than the spike it serves.
// ---------------------------------------------------------------------------

/// Protobuf varint.
fn varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// A varint field: `(field << 3) | 0`.
fn put_varint(out: &mut Vec<u8>, field: u32, value: u64) {
    varint(out, u64::from(field) << 3);
    varint(out, value);
}

/// A length-delimited field: `(field << 3) | 2`, a length, then the bytes.
/// Strings and nested messages are the same shape on the wire.
fn put_bytes(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    varint(out, (u64::from(field) << 3) | 2);
    varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// `TypeProto` for a float tensor of the given fixed shape.
fn float_tensor_type(dims: &[i64]) -> Vec<u8> {
    // TensorShapeProto { repeated Dimension dim = 1 }
    let mut shape = Vec::new();
    for d in dims {
        let mut dim = Vec::new();
        put_varint(&mut dim, 1, *d as u64); // Dimension.dim_value
        put_bytes(&mut shape, 1, &dim);
    }

    // TypeProto.Tensor { int32 elem_type = 1; TensorShapeProto shape = 2 }
    let mut tensor = Vec::new();
    put_varint(&mut tensor, 1, 1); // TensorProto.DataType.FLOAT
    put_bytes(&mut tensor, 2, &shape);

    // TypeProto { Tensor tensor_type = 1 }
    let mut ty = Vec::new();
    put_bytes(&mut ty, 1, &tensor);
    ty
}

/// `ValueInfoProto` — a named graph input or output.
fn value_info(name: &str, dims: &[i64]) -> Vec<u8> {
    let mut info = Vec::new();
    put_bytes(&mut info, 1, name.as_bytes()); // name
    put_bytes(&mut info, 2, &float_tensor_type(dims)); // type
    info
}

/// A complete ONNX model computing `c = a + b` over three floats.
///
/// IR version 7 with opset 13 — old enough that every ONNX Runtime since 1.8
/// accepts it, which matters because the point is to test *this* build's
/// runtime, not to discover an opset mismatch.
pub fn tiny_add_model() -> Vec<u8> {
    // NodeProto { input = 1, output = 2, name = 3, op_type = 4 }
    let mut node = Vec::new();
    put_bytes(&mut node, 1, b"a");
    put_bytes(&mut node, 1, b"b");
    put_bytes(&mut node, 2, b"c");
    put_bytes(&mut node, 3, b"add");
    put_bytes(&mut node, 4, b"Add");

    // GraphProto { node = 1, name = 2, input = 11, output = 12 }
    let mut graph = Vec::new();
    put_bytes(&mut graph, 1, &node);
    put_bytes(&mut graph, 2, b"relay_onnx_spike");
    put_bytes(&mut graph, 11, &value_info("a", &[3]));
    put_bytes(&mut graph, 11, &value_info("b", &[3]));
    put_bytes(&mut graph, 12, &value_info("c", &[3]));

    // OperatorSetIdProto { domain = 1, version = 2 }. The default domain is
    // the empty string, and an omitted field already means that, so only the
    // version is written.
    let mut opset = Vec::new();
    put_varint(&mut opset, 2, 13);

    // ModelProto { ir_version = 1, producer_name = 2, graph = 7, opset_import = 8 }
    let mut model = Vec::new();
    put_varint(&mut model, 1, 7);
    put_bytes(&mut model, 2, b"relay-onnx-spike");
    put_bytes(&mut model, 7, &graph);
    put_bytes(&mut model, 8, &opset);
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal protobuf reader, so the encoder can be checked against
    /// something other than itself.
    ///
    /// This proves the bytes are *well-formed protobuf carrying the fields
    /// this file thinks it wrote*. It cannot prove ONNX Runtime accepts them —
    /// only the Windows run can — but it does rule out the failure that would
    /// waste that run: a malformed fixture read as "ONNX did not work".
    fn fields(mut buf: &[u8]) -> Vec<(u32, Field<'_>)> {
        let mut out = Vec::new();
        while !buf.is_empty() {
            let (key, rest) = read_varint(buf);
            let field = (key >> 3) as u32;
            let wire = key & 0x7;
            buf = rest;
            match wire {
                0 => {
                    let (value, rest) = read_varint(buf);
                    buf = rest;
                    out.push((field, Field::Varint(value)));
                }
                2 => {
                    let (len, rest) = read_varint(buf);
                    let len = len as usize;
                    assert!(len <= rest.len(), "length {len} overruns {} bytes", rest.len());
                    out.push((field, Field::Bytes(&rest[..len])));
                    buf = &rest[len..];
                }
                other => panic!("unexpected wire type {other} for field {field}"),
            }
        }
        out
    }

    enum Field<'a> {
        Varint(u64),
        Bytes(&'a [u8]),
    }

    impl<'a> Field<'a> {
        fn varint(&self) -> u64 {
            match self {
                Field::Varint(v) => *v,
                Field::Bytes(_) => panic!("expected a varint"),
            }
        }
        fn bytes(&self) -> &'a [u8] {
            match self {
                Field::Bytes(b) => b,
                Field::Varint(_) => panic!("expected a length-delimited field"),
            }
        }
    }

    fn read_varint(buf: &[u8]) -> (u64, &[u8]) {
        let mut value = 0u64;
        let mut shift = 0;
        for (i, byte) in buf.iter().enumerate() {
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return (value, &buf[i + 1..]);
            }
            shift += 7;
            assert!(shift < 64, "varint longer than 64 bits");
        }
        panic!("truncated varint");
    }

    fn only<'a>(fields: &'a [(u32, Field<'a>)], number: u32) -> &'a Field<'a> {
        let matches: Vec<_> = fields.iter().filter(|(n, _)| *n == number).collect();
        assert_eq!(matches.len(), 1, "expected exactly one field {number}");
        &matches[0].1
    }

    fn all<'a>(fields: &'a [(u32, Field<'a>)], number: u32) -> Vec<&'a Field<'a>> {
        fields
            .iter()
            .filter(|(n, _)| *n == number)
            .map(|(_, f)| f)
            .collect()
    }

    #[test]
    fn the_model_is_a_well_formed_protobuf_message() {
        let bytes = tiny_add_model();
        let model = fields(&bytes);
        assert_eq!(only(&model, 1).varint(), 7, "ir_version");
        assert_eq!(only(&model, 2).bytes(), b"relay-onnx-spike");
        assert_eq!(only(&fields(only(&model, 8).bytes()), 2).varint(), 13, "opset");
    }

    #[test]
    fn the_graph_adds_its_two_inputs_into_one_output() {
        let bytes = tiny_add_model();
        let model = fields(&bytes);
        let graph = fields(only(&model, 7).bytes());

        assert_eq!(only(&graph, 2).bytes(), b"relay_onnx_spike");

        let node = fields(only(&graph, 1).bytes());
        assert_eq!(only(&node, 4).bytes(), b"Add", "op_type");
        let inputs: Vec<_> = all(&node, 1).iter().map(|f| f.bytes()).collect();
        assert_eq!(inputs, vec![b"a".as_slice(), b"b".as_slice()]);
        assert_eq!(only(&node, 2).bytes(), b"c", "output");
    }

    /// Every name the node uses is declared, with a shape, and every declared
    /// shape is the one the spike's fixed inputs actually have.
    ///
    /// A graph whose input names do not match what `infer` passes fails at
    /// runtime with a message about a missing input, which reads exactly like
    /// a packaging failure and is not one.
    #[test]
    fn the_declared_inputs_match_what_the_spike_feeds_them() {
        let bytes = tiny_add_model();
        let model = fields(&bytes);
        let graph = fields(only(&model, 7).bytes());

        let declared = |field: u32| -> Vec<(String, Vec<u64>)> {
            all(&graph, field)
                .iter()
                .map(|info| {
                    let info = fields(info.bytes());
                    let name = String::from_utf8(only(&info, 1).bytes().to_vec()).expect("name");
                    let ty = fields(only(&info, 2).bytes());
                    let tensor = fields(only(&ty, 1).bytes());
                    assert_eq!(only(&tensor, 1).varint(), 1, "{name} should be float");
                    let dims = all(&fields(only(&tensor, 2).bytes()), 1)
                        .iter()
                        .map(|d| only(&fields(d.bytes()), 1).varint())
                        .collect();
                    (name, dims)
                })
                .collect()
        };

        assert_eq!(
            declared(11),
            vec![
                ("a".to_string(), vec![INPUT_A.len() as u64]),
                ("b".to_string(), vec![INPUT_B.len() as u64]),
            ]
        );
        assert_eq!(
            declared(12),
            vec![("c".to_string(), vec![EXPECTED.len() as u64])]
        );
    }

    /// The arithmetic the Windows run is checked against, checked here.
    #[test]
    fn the_expected_output_is_the_sum_of_the_inputs() {
        let summed: Vec<f32> = INPUT_A
            .iter()
            .zip(INPUT_B.iter())
            .map(|(a, b)| a + b)
            .collect();
        assert_eq!(summed, EXPECTED.to_vec());
    }

    /// Without the feature the spike still produces a report, and that report
    /// says why it is empty rather than looking like a passing run.
    #[cfg_attr(feature = "onnx-spike", ignore = "ort is compiled in")]
    #[test]
    fn a_build_without_the_feature_reports_that_and_not_success() {
        let report = run();
        assert!(!report.ort_compiled_in);
        assert!(report.inference.is_none());
        assert!(report.error.is_some());
        assert!(report.model_bytes > 0);
    }
}
