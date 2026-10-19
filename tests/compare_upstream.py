#!/usr/bin/env python3
"""Exact synthetic read_vector comparisons; reports are HDF5, never RF packs."""
import argparse
import json
import pathlib
import subprocess
import tempfile

import digital_rf
import h5py
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=pathlib.Path, required=True)
    parser.add_argument('--report', type=pathlib.Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if args.report.exists():
        parser.error('Choose a new report path; existing reports are not overwritten')
    if digital_rf.__version__ != '2.6.14':
        parser.error('This parity matrix is pinned to digital_rf==2.6.14')
    results = []
    with tempfile.TemporaryDirectory(prefix='rdigital-rf-synthetic-') as tmp:
        directory = pathlib.Path(tmp)
        for component in ['i1', 'i2', 'i4', 'i8', 'u1', 'u2', 'u4', 'u8', 'f4', 'f8']:
            for complex_data in [False, True]:
                case = component + ('-complex' if complex_data else '-real')
                dtype = np.dtype([('r', component), ('i', component)]) if complex_data else np.dtype(component)
                roots = []
                for index, start in enumerate([35990, 36000]):
                    root = directory / case / str(index)
                    channel = root / 'ch'
                    channel.mkdir(parents=True)
                    roots.append(str(root))
                    with digital_rf.DigitalRFWriter(
                        str(channel), dtype, 3600, 1000, start, 10, 1,
                        compression_level=1, checksum=True,
                        is_complex=complex_data, num_subchannels=2,
                        is_continuous=False,
                    ) as writer:
                        data = np.zeros((10, 2), dtype=dtype)
                        values = np.arange(index * 20, index * 20 + 20).reshape(10, 2)
                        if complex_data:
                            data['r'], data['i'] = values, values + 40
                        else:
                            data[:] = values
                        writer.rf_write(data)
                reference = digital_rf.DigitalRFReader(roots, rdcc_nbytes=64 * 1024 * 1024)
                for label, start, length, failure in [
                    ('one-file', 35991, 4, False),
                    ('root-hour-boundary', 35995, 10, False),
                    ('missing-file', 36005, 10, True),
                    ('single-row-squeeze', 35991, 1, True),
                    ('zero-length', 35991, 0, True),
                ]:
                    name = case + '-' + label
                    try:
                        expected = reference.read_vector(start, length, 'ch')
                        reference_error = False
                    except (IOError, ValueError):
                        reference_error = True
                    output = directory / (name + '.h5')
                    native = subprocess.run(
                        [str(binary), str(output), 'ch', str(start), str(length), *roots],
                        text=True, capture_output=True,
                    )
                    assert reference_error == (native.returncode != 0) == failure, (
                        name, reference_error, native.returncode, native.stderr,
                    )
                    row = dict(name=name, matched_error=failure)
                    if not failure:
                        with h5py.File(output) as handle:
                            pairs = handle['vector'][:]
                            shape = tuple(handle['shape'][:])
                            actual = (pairs[:, 0] + 1j * pairs[:, 1]).reshape(shape)
                            native_dtype = handle.attrs['format']
                            if isinstance(native_dtype, np.ndarray):
                                native_dtype = native_dtype.tobytes().decode()
                            if isinstance(native_dtype, bytes):
                                native_dtype = native_dtype.decode()
                            assert np.dtype(native_dtype) == expected.dtype, (name, native_dtype, expected.dtype)
                            np.testing.assert_array_equal(actual, expected, err_msg=name)
                            assert shape == expected.shape, name
                            row.update(shape=list(map(int, shape)), dtype=str(expected.dtype), exact=True,
                                       cache_bytes=int(handle.attrs['cache_bytes']))
                    results.append(row)
                del reference
    args.report.parent.mkdir(parents=True, exist_ok=True)
    payload = np.frombuffer(json.dumps(results).encode(), np.uint8)
    with h5py.File(args.report, 'x') as handle:
        handle.attrs['reference'] = 'digital_rf ' + digital_rf.__version__
        handle.attrs['fixture_kind'] = 'synthetic compressed two-root two-subchannel recordings'
        handle['checks_json'] = payload
    print(f'{len(results)} exact value/shape/dtype or matching-error checks passed; {args.report}')


if __name__ == '__main__':
    main()
