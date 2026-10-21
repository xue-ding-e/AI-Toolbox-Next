"""Format only the added test function; preserve every pre-existing source byte."""
import pathlib, subprocess, tempfile
path=pathlib.Path('tauri/src/settings/backup/utils.rs')
base=subprocess.check_output(['git','show','HEAD:'+str(path)])
current=path.read_bytes()
needle=b'    #[test]\n    fn restore_old_schema_snapshot_migrates_missing_tables()'
assert base.count(needle)==1
prefix,suffix=base.split(needle,1)
suffix=needle+suffix
assert current.startswith(prefix) and current.endswith(suffix)
added=current[len(prefix):-len(suffix)]
assert added.startswith(b'    #[test]\n    fn restore_migration_failure_rolls_back_synthetic_database()')
with tempfile.TemporaryDirectory(prefix='ai-format-test-') as tmp:
    function=pathlib.Path(tmp)/'candidate.rs'
    function.write_bytes(added)
    subprocess.run(['rustfmt','+1.99.1','--edition','2021','--config','skip_children=true',str(function)],check=True,timeout=30)
    # Standalone function is indented back into its existing tests module.
    formatted=b'\n'.join((b'    '+line if line else b'') for line in function.read_bytes().splitlines())+b'\n\n'
path.write_bytes(prefix+formatted+suffix)
assert path.read_bytes()[:len(prefix)]==prefix
assert path.read_bytes()[-len(suffix):]==suffix
assert path.read_bytes().replace(formatted,b'',1)==base
