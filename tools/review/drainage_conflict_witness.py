"""Extract a positive numerical Farkas witness from planets constraint dumps.
Usage: python drainage_conflict_witness.py DUMP_DIRECTORY OUTPUT_JSON
Requires NumPy/SciPy; run under the shared scenery launcher.
"""
import sys,json,hashlib
from pathlib import Path
import numpy as np
from scipy import sparse
from scipy.optimize import linprog
root=Path(sys.argv[1]); z=np.load(root/'constraint_system.npz'); meta=json.loads((root/'constraint_rows.json').read_text())
A=sparse.csr_matrix((z['A_data'],z['A_indices'],z['A_indptr']),shape=z['A_shape']); b=z['rhs']
scale=1/np.maximum(np.asarray(abs(A).max(axis=1).toarray()).ravel(),1e-12)
S=sparse.diags(scale)@A; d=scale*b
E=sparse.vstack([S.T,sparse.csr_matrix(np.ones((1,A.shape[0])))],format='csr')
r=linprog(d,A_eq=E,b_eq=np.r_[np.zeros(A.shape[1]),1],bounds=(0,None),method='highs',options={'primal_feasibility_tolerance':1e-9,'dual_feasibility_tolerance':1e-9,'time_limit':90})
print(r.message,flush=True)
if not r.success: sys.exit(1)
y=r.x*scale; ids=np.flatnonzero(r.x>0)
out=dict(source=str(root),sha256=hashlib.sha256((root/'constraint_system.npz').read_bytes()).hexdigest(),b_dot_y=float(b@y),max_At_y=float(np.max(abs(A.T@y))),support=[dict(row=int(i),weight=float(y[i]),scaled_weight=float(r.x[i]),rhs=float(b[i]),provenance=meta[i],vertices=z['variable'][A[i].indices].tolist(),coefficients=A[i].data.tolist()) for i in ids])
print(json.dumps(out,indent=2)); Path(sys.argv[2]).write_text(json.dumps(out,indent=2)+'\n')
