"""Format only added decoder/helper tests; assert every other byte matches reviewed input."""
import pathlib, subprocess, tempfile
p=pathlib.Path('tauri/src/coding/ssh/session.rs');s=p.read_bytes();expected=s
segments=[(b'/// Display-only diagnostic fallback adapted',b'/// SSH ',False),(b'    #[test]\n    fn remote_diagnostic_keeps_utf8_and_empty()',b'    #[test]\n    fn matches_excluded_directory_by_name_segment_only()',True)]
for start,end,indent in segments:
 a=s.index(start);b=s.index(end,a);old=s[a:b]
 with tempfile.TemporaryDirectory(prefix='ai-ssh-format-') as tmp:
  f=pathlib.Path(tmp)/'candidate.rs';f.write_bytes(old)
  subprocess.run(['rustfmt','+1.99.1','--edition','2021','--config','skip_children=true',str(f)],check=True,timeout=30)
  new=f.read_bytes().rstrip()+b'\n\n'
  if indent:new=b'\n'.join((b'    '+line if line else b'') for line in new.splitlines())+b'\n\n'
 s=s[:a]+new+s[b:]
 expected=expected.replace(old,b'',1)
# Removing only the new formatted segments must preserve all other candidate bytes.
rest=s
for start,end,_ in segments:
 a=rest.index(start);b=rest.index(end,a);rest=rest[:a]+rest[b:]
assert rest==expected
p.write_bytes(s)
