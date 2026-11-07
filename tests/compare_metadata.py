"""Compare Rust reads with fixtures produced and read by upstream Digital RF."""
import os
from pathlib import Path
import subprocess
import tempfile
import digital_rf
import numpy as np

with tempfile.TemporaryDirectory() as directory:
    start = 1_700_003_599_000_000
    writer = digital_rf.DigitalMetadataWriter(directory, 3600, 60, 1_000_000, 1, "mf")
    for i in range(2):
        key = start + i * 2_000_000
        writer.write([key], {
            "tx_idxs": [np.array([9_007_199_254_740_993, key], dtype=np.int64)],
            "max_snr": [np.array([2.5, 9.0], dtype=np.float32)],
            "beam": [np.array([0, 4], dtype=np.int8)],
            "scalar": np.float64(3.25),
        })
    reference = digital_rf.DigitalMetadataReader(directory).read(start, start + 2_000_000)
    assert list(reference) == [start, start + 2_000_000]
    for item in reference.values():
        np.testing.assert_array_equal(item["max_snr"], np.array([2.5, 9], dtype=np.float32))
        assert item["tx_idxs"][0] == 9_007_199_254_740_993
    subprocess.run(["cargo", "test", "--test", "metadata", "upstream_writer_fixture", "--", "--ignored"],
                   cwd=Path(__file__).resolve().parents[1],
                   env={**os.environ, "DIGITAL_METADATA_FIXTURE": directory}, check=True)
