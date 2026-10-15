#!/usr/bin/env bash
# Records what a check actually opens, by watching the two things that can open
# an archive here.
#
# Round 5's review found the mechanism this file replaces. `derivation.reads` was
# derived — but from filenames. `checks/derivation-reads.sh` walks a producer's
# `. "$checks_dir/x.sh"` lines and unions what `checks/derivation-vocabulary.json`
# says each of those names reads. Every link in that chain is a name. Nothing
# read a check's body, so a reader of 1,113 class-file bodies was injected into
# `checks/surfaces-model.sh`, no vocabulary entry and no record was touched, and
# the gate exited 0 with every record still declaring `decompiled: false`,
# `restrictedContentRead: false` and `implementationMethod: metadata-only`.
#
# That sentence is the one that makes reading 235 restricted-licence artifacts a
# lawful thing to have done, so it has to become true rather than be softened,
# and no table of phrases can make it true: a phrase is a claim about code, and
# the code is what has to be asked. So it is asked. This file wraps the two
# channels this workspace is known to open archives through — Python's `zipfile`
# and the `unzip` binary:
#
#   * a `sitecustomize.py` on PYTHONPATH patches `ZipFile.namelist`,
#     `ZipFile.infolist`, `ZipFile.open` and `ZipExtFile.read`, so every listing
#     and every byte count is counted where the standard library performs it;
#   * an `unzip` earlier on PATH records its own invocation and then execs the
#     real binary at the absolute path resolved here, so the shim cannot find
#     itself.
#
# Neither wrapper decides anything. They write down the archive, the entry and
# how much of it was read; `checks/verify-reads.sh` is where that is judged.
#
# What this does not see, and it is more than the file used to say. What is
# watched is four methods of one standard-library class and one binary on PATH.
# It is not "the standard library rather than the caller": `open`, `zlib`, `io`,
# `mmap`, `subprocess`, `os.read`, `sha256sum`, `dd` and `cat` are all standard
# and none of them is watched, and an interpreter started with -S never loads the
# sitecustomize at all. Round 6's review proved both halves of that — a reader
# using `open`, `struct` and `zlib.decompress` inflated 50,357 class files under
# a green gate, and the same injection through `zipfile` verbatim was invisible
# under `python3 -S`.
#
# So the claim that those two channels are the only ones is no longer made here
# and believed nowhere. It is read out of the code by
# `checks/verify-primitives.sh`, which refuses the unwatched primitives outright
# and requires every legitimate use of the rest to be named in a table it owns;
# and the interpreter is shimmed below so the isolation flags are refused at run
# time as well as in the text. Neither of those makes the premise true by
# construction. Together they mean a hand has to work at breaking it rather than
# simply not think about it.
#
# The other thing this does not see: reads of files that are not archives — this
# workspace's own tracked files, the pinned manifest, the acquisition log, the
# pack override tree. Those are named in the vocabulary and are not observed.
#
# Sourcing this defines read_observation_begin, which prepares the shims and
# exports the environment they key on. Call it in a subshell: it edits PATH.
set -euo pipefail

# Where the shims write. One line per invocation for unzip, which is atomic at
# this length; one file per process for Python, which writes its whole tally at
# exit.
read_observation_begin() {
  local dir="$1"
  local real_unzip real_python
  if ! real_unzip="$(command -v unzip)"; then
    echo "read observation requires unzip" >&2
    return 1
  fi
  if ! real_python="$(command -v python3)"; then
    echo "read observation requires python3" >&2
    return 1
  fi

  rm -rf "$dir"
  mkdir -p "$dir/bin" "$dir/py" "$dir/log"

  cat >"$dir/bin/unzip" <<EOF
#!/usr/bin/env bash
# Written by checks/read-observation.sh. Records the invocation, then runs the
# real binary at the path resolved when the observation began.
if [ "\${1:-}" = "-Z1" ]; then
  printf 'READ\tunzip\t%s\t\ttoc\t1\n' "\${2:-}" >>"\$RAVEL_READ_DIR/unzip.log"
elif [ "\${1:-}" = "-p" ]; then
  printf 'READ\tunzip\t%s\t%s\tbytes:full\t1\n' "\${2:-}" "\${3:-}" >>"\$RAVEL_READ_DIR/unzip.log"
else
  printf 'READ\tunzip\t%s\t%s\tunrecognised\t1\n' "\${1:-}" "\${*:2}" >>"\$RAVEL_READ_DIR/unzip.log"
fi
exec $real_unzip "\$@"
EOF
  chmod +x "$dir/bin/unzip"

  # The interpreter, shimmed for one reason: the Python half of this observation
  # is a sitecustomize, and an interpreter started with -S, -E or -I never loads
  # it. Round 6's review ran the round 5 injection verbatim, through zipfile and
  # nothing else, under `python3 -S`, and the observation saw none of it while
  # the gate printed that it had classified 1.4 million reads. The static half of
  # this rule is `checks/verify-primitives.sh`, which refuses those flags
  # anywhere under checks/; this is the same rule at run time, so an invocation
  # reaching the interpreter from generated shell is refused too. The leading
  # options are inspected and the scan stops at the program, so a -c program
  # containing an S is not a flag.
  # It also records which script it was pointed at, and that is not decoration.
  # `checks/verify-primitives.sh` accounts for the invocation in
  # `checks/mod-metadata.sh` and cannot account for its argument, which is a
  # variable: repointing `reader=` at a file the scan never read would leave the
  # table row true and the reader unseen. So the interpreter says what it ran and
  # `checks/verify-reads.sh` requires it to be a file under checks/ in the
  # sandbox. A -c program is a file nothing holds, so it says so and fails there.
  cat >"$dir/bin/python3" <<EOF
#!/usr/bin/env bash
# Written by checks/read-observation.sh.
ravel_script=
for arg in "\$@"; do
  case "\$arg" in
    -*[SEI]*)
      printf 'ISOLATED\t%s\n' "\$*" >>"\$RAVEL_READ_DIR/python3.log"
      ;;
  esac
  case "\$arg" in
    -c | -m) ravel_script='<inline>'; break ;;
    -*) ;;
    *) ravel_script="\$arg"; break ;;
  esac
done
printf 'SCRIPT\t%s\n' "\${ravel_script:-<repl>}" >>"\$RAVEL_READ_DIR/python3.log"
exec $real_python "\$@"
EOF
  chmod +x "$dir/bin/python3"

  cat >"$dir/py/sitecustomize.py" <<'EOF'
"""Written by checks/read-observation.sh.

Counts every archive listing and every byte read through Python's own zipfile,
at the point the standard library performs it. The tally is kept in memory and
written once at exit, so a reader of 71,000 entries costs one file.
"""

import atexit
import os
import zipfile

_dir = os.environ.get("RAVEL_READ_DIR")
if _dir:
    _tally: dict[tuple[str, str, str], int] = {}

    def _count(archive, member, extent):
        key = (archive or "<in-memory>", member or "", extent)
        _tally[key] = _tally.get(key, 0) + 1

    _namelist = zipfile.ZipFile.namelist
    _infolist = zipfile.ZipFile.infolist
    _zipopen = zipfile.ZipFile.open
    _extread = zipfile.ZipExtFile.read

    def namelist(self):
        _count(self.filename, "", "toc")
        return _namelist(self)

    def infolist(self):
        _count(self.filename, "", "toc")
        return _infolist(self)

    def zipopen(self, name, *args, **kwargs):
        handle = _zipopen(self, name, *args, **kwargs)
        try:
            handle._ravel_archive = self.filename
        except AttributeError:
            pass
        return handle

    def extread(self, *args, **kwargs):
        data = _extread(self, *args, **kwargs)
        _count(getattr(self, "_ravel_archive", None), getattr(self, "name", ""),
               "bytes:%d" % len(data))
        return data

    zipfile.ZipFile.namelist = namelist
    zipfile.ZipFile.infolist = infolist
    zipfile.ZipFile.open = zipopen
    zipfile.ZipExtFile.read = extread

    @atexit.register
    def _flush():
        if not _tally:
            return
        path = os.path.join(_dir, "python.%d" % os.getpid())
        with open(path, "a", encoding="utf-8") as handle:
            for (archive, member, extent), count in sorted(_tally.items()):
                handle.write("READ\tzipfile\t%s\t%s\t%s\t%d\n"
                             % (archive, member, extent, count))
EOF

  : >"$dir/log/unzip.log"
  : >"$dir/log/python3.log"

  # The three variables the shims key on. A command run without RAVEL_READ_DIR
  # is not observed at all and says so by leaving an empty log, which
  # verify-reads.sh refuses rather than reads as a quiet pass.
  RAVEL_READ_DIR="$dir/log"
  PATH="$dir/bin:$PATH"
  PYTHONPATH="$dir/py"
  export RAVEL_READ_DIR PATH PYTHONPATH
}
