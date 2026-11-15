"""Format the checked HDF5 benchmark report as a LaTeX table."""
from pathlib import Path
import h5py

CASES = [
    ('HDF5, uncompressed', 'i8', 'direct', 'hdf8_native'),
    ('HDF5, uncompressed', 'i8', 'vector', 'hdf8_vector'),
    ('HDF5, gzip-1', 'i8', 'direct', 'gzip8_default'),
    ('HDF5, gzip-1, 128 KiB cache', 'i8', 'direct', 'gzip8_128k_cache'),
    ('HDF5, uncompressed', 'i16', 'direct', 'hdf16_native'),
    ('Raw', 'i8', 'direct', 'raw8_native'),
    ('Raw', 'i8', 'vector', 'raw8_vector'),
    ('Raw', 'i16', 'direct', 'raw16_native'),
]
root = Path(__file__).resolve().parent
lines = [r'% Numerical report and source provenance are retained in HDF5 metadata.',
         r'\begin{tabularx}{\linewidth}{@{}Xllrr@{}}\toprule',
         r'Container / cache & Type & API & Full & Sparse\\\midrule']
with h5py.File(root / 'feature-matrix.h5', 'r') as report:
    for label, component, api, key in CASES:
        full = report[key + '_contiguous']
        sparse = report[key + '_sparse']
        assert full['exact_equal'][()] == sparse['exact_equal'][()] == 1
        a = full['median_seconds'][()] * 1000
        b = sparse['median_seconds'][()] * 1000
        lines.append(f'{label} & {component} & {api} & {a:.2f} & {b:.2f} ' + r'\\')
lines.extend([r'\bottomrule', r'\end{tabularx}'])
(root / 'table.tex').write_text('\n'.join(lines) + '\n')
