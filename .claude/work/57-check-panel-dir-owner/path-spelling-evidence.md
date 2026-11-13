# Symlink spelling experiment

Python tempfile: create owned directory mode 0700, alias symlink to it, call os.lstat on the three spellings. Linux result:
```text
lstat alias   symlink=True  directory=False
lstat alias/  symlink=False directory=True
lstat alias/. symlink=False directory=True
```
This proves raw lstat alone does not meet AC4 for terminal slash/dot spellings. The implementation must inspect the final meaningful path component without resolving symlinks (strip only terminal slash/dot components, preserve parent traversal), or fail closed for those spellings. Add explicit cases in Rust and JS tests. This experiment modified only an automatically removed temporary directory, not project code.
