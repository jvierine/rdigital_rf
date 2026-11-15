from pathlib import Path
import h5py,numpy as np,hashlib
root=Path(__file__).resolve().parent
repo=root.parents[1]
paths=[root/f'audit-run-{i}.h5' for i in [1,2,3]]
with h5py.File(root/'feature-matrix.h5','w') as out:
 with h5py.File(paths[0]) as first:
  for k,v in first.attrs.items(): out.attrs[k]=v
 out.attrs['method']='Three fresh processes, each with one warm-up and five timed repeats per case; aggregate medians over 15 repeats. Audit pairs alternate order; shared conversion reuses one destination buffer.'
 out.attrs['cpu_model']='Intel Core i9-11950H @ 2.60 GHz; ext4; Linux x86-64'
 out.attrs['rustc']='rustc 1.99.0 (b940084d7 2026-09-28)'
 out.attrs['hdf5_version']='1.10.10'
 out.attrs['repeats']=15
 out.attrs['benchmark_source_sha256']=hashlib.sha256((repo/'examples/bench_features.rs').read_bytes()).hexdigest()
 out.attrs['raw_source_sha256']=hashlib.sha256((repo/'src/raw.rs').read_bytes()).hexdigest()
 out.attrs['typed_source_sha256']=hashlib.sha256((repo/'src/typed.rs').read_bytes()).hexdigest()
 out.attrs['reader_source_sha256']=hashlib.sha256((repo/'src/lib.rs').read_bytes()).hexdigest()
 with h5py.File(paths[0]) as first: keys=list(first)
 for key in keys:
  times=[]; metadata={}
  for path in paths:
   with h5py.File(path) as h:
    g=h[key];assert g['exact_equal'][()]==1
    times.extend(g['seconds'][:])
    for k in g:
     if k not in ['seconds','median_seconds','min_seconds','max_seconds','million_samples_per_second']: metadata[k]=g[k][()]
  g=out.create_group(key);g['seconds']=times;g['median_seconds']=np.median(times);g['min_seconds']=min(times);g['max_seconds']=max(times)
  for k,v in metadata.items(): g[k]=v
  if 'samples_per_repeat' in metadata:g['million_samples_per_second']=metadata['samples_per_repeat']/np.median(times)/1e6
  print(key,round(float(np.median(times))*1000,3))
