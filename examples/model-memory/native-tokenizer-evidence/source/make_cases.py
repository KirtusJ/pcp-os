from pathlib import Path
import ast, random, hashlib, struct
import sys
output=Path(sys.argv[2])
src=Path(sys.argv[1])
func=next(x for x in ast.parse(src.read_text()).body if isinstance(x,ast.FunctionDef) and x.name=='main')
body=next(x for x in func.body if isinstance(x,ast.Try)).body
start=next(i for i,x in enumerate(body) if isinstance(x,ast.Assign) and isinstance(x.targets[0],ast.Name) and x.targets[0].id=='cases')
end=next(i for i,x in enumerate(body) if isinstance(x,ast.Assign) and isinstance(x.targets[0],ast.Name) and x.targets[0].id=='lines')
scope={'random':random}
exec(compile(ast.Module(body[start:end],[]),str(src),'exec'),scope)
cases=[x.encode() for x in scope['cases']]
assert len(cases)==541
output.write_bytes(b''.join(struct.pack('<H',len(b))+b for b in cases))
