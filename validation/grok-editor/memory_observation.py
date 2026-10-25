"""Pure hosted visible-v2 memory observation; no resource admission decision.

Adapted from reviewed AI440 guard in 92198aae38b1b3efb3bfcd065241f843a424791c,
.github/scripts/verify-codex-usage-440.py:admit. Only own-process namespace and
visible ancestry are observed. Unlike old Grok r4, no PID1 observation is made.
This is the standard non-container GitHub-hosted VM assurance model, not a
hostile-container proof, a relaxation of permissions, or a diagnosis of the old
unlocalized errno13. Denied/unknown observations stop without another route.

The caller retains initial capacity/headroom, runtime RSS/low-water, deadlines,
and owned-process cleanup gates. Low numerical observations are returned, not
rounded up; this module performs no disk check and grants no build permission.
"""
import errno
import os
from pathlib import Path
import re
import stat


class ObservationError(RuntimeError):
    """Safe failed-operation metadata; raw OS messages are not displayed."""
    def __init__(self, read_class, error_number, reason):
        self.read_class = read_class
        self.errno = error_number
        self.reason = reason
        super().__init__(f"MEMORY_OBSERVATION_UNKNOWN: {read_class}: {reason}")

    def diagnostic(self):
        return {"read_class": self.read_class, "errno": self.errno, "reason": self.reason}


def observe_memory():
    """Owned-process/visible-v2 observation, only on the configured hosted VM.

    Never reads PID 1 or another process's namespace. This is not a generic
    hostile-container capacity proof. Unsupported layouts and denied reads stop.
    """
    def require(condition, message):
        if not condition:
            raise ObservationError("validation", None, message)

    def record_read(label, operation):
        try:
            return operation()
        except OSError as error:
            # No success logging or retries. Never include the OS error string,
            # which may contain a host path. Exact ENOENT is handled only at the
            # two root scalar controls below; every other failure propagates.
            raise ObservationError(label, error.errno, "metadata read failed") from None

    require(os.environ.get("GITHUB_ACTIONS") == "true"
            and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
            and os.environ.get("RUNNER_OS") == "Linux", "requires the configured standard GitHub-hosted Linux VM")
    info = record_read("proc-meminfo", lambda: Path("/proc/meminfo").read_text())
    fields = {}
    for line in info.splitlines():
        key, _, value = line.partition(":")
        if key in ("MemTotal", "MemAvailable"):
            parts = value.split()
            require(len(parts) == 2 and parts[1] == "kB" and parts[0].isdigit(), "invalid memory units")
            fields[key] = int(parts[0]) * 1024
    require(set(fields) == {"MemTotal", "MemAvailable"}, "missing host memory metrics")
    require(0 <= fields["MemAvailable"] <= fields["MemTotal"], "invalid available memory")
    cgroup = record_read("proc-self-cgroup", lambda: Path("/proc/self/cgroup").read_text())
    mounts = record_read("proc-self-mountinfo", lambda: Path("/proc/self/mountinfo").read_text())
    namespace = record_read("proc-self-cgroup-namespace", lambda: os.readlink("/proc/self/ns/cgroup"))
    require(bool(re.fullmatch(r"cgroup:\[[0-9]+\]", namespace)), "unknown own namespace")
    entries = cgroup.splitlines()
    require(len(entries) == 1 and entries[0].startswith("0::"), "v1/mixed cgroups unsupported")
    relative = entries[0][3:]

    def safe_absolute(value):
        require(value.startswith("/") and "\\" not in value and "\0" not in value,
                "unsupported path encoding")
        require(value == "/" or all(part not in ("", ".", "..") for part in value[1:].split("/")), "unsafe cgroup path")
        return value

    safe_absolute(relative)
    found, points = [], []
    for line in mounts.splitlines():
        left, separator, right = line.partition(" - ")
        require(bool(separator), "invalid mountinfo")
        columns = left.split()
        require(len(columns) >= 6, "invalid mount columns")
        kinds = right.split()
        require(bool(kinds), "missing mount filesystem type")
        kind = kinds[0]
        require(kind != "cgroup", "mixed/v1 hierarchy unsupported")
        points.append(safe_absolute(columns[4]))
        if kind == "cgroup2":
            require(columns[3] == "/", "hidden cgroup ancestors")
            found.append(safe_absolute(columns[4]))
    require(len(found) == 1, "ambiguous or missing v2 mount")
    mount = found[0]
    require(not any(point.startswith(mount + "/") for point in points), "nested mount hides ancestry")
    require(record_read("cgroup-v2-root-resolution", lambda: str(Path(mount).resolve(strict=True))) == mount, "cgroup mount alias")
    components = relative.strip("/").split("/") if relative != "/" else []
    require(len(components) <= 128, "cgroup hierarchy too deep")
    descriptor = record_read("cgroup-v2-root-directory", lambda: os.open(mount, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW))
    records = []

    def control(fd, name, depth, maximum=128):
        label = f"cgroup-v2-depth-{depth}-{name}"
        def read():
            child = record_read(label + "-open", lambda: os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd))
            try:
                require(stat.S_ISREG(os.fstat(child).st_mode), "nonregular control")
                data = os.read(child, maximum + 1)
                require(len(data) <= maximum, "oversized control")
                return data.decode("ascii").strip()
            finally:
                os.close(child)
        return record_read(label, read)

    try:
        for depth in range(len(components) + 1):
            before = record_read(f"cgroup-v2-depth-{depth}-directory-stat-before", lambda: os.fstat(descriptor))
            if depth == 0:
                require("memory" in control(descriptor, "cgroup.controllers", depth).split(), "memory controller unavailable")
                root_stat = control(descriptor, "memory.stat", depth, 65536)
                names = set()
                for line in root_stat.splitlines():
                    parts = line.split()
                    require(len(parts) == 2 and bool(re.fullmatch(r"[a-z][a-z0-9_]*", parts[0])) and parts[1].isdigit(), "invalid root memory.stat")
                    require(parts[0] not in names, "duplicate root memory.stat field")
                    names.add(parts[0])
                require({"anon", "file", "kernel_stack", "pagetables"} <= names, "incomplete root memory.stat")
                # On this ordinary VM, positive memory.stat plus BOTH scalar
                # controls absent identifies the real kernel root, not a hidden
                # namespaced child. An EACCES is never treated as absence.
                for name in ("memory.max", "memory.current"):
                    try:
                        control(descriptor, name, depth)
                    except ObservationError as error:
                        if error.errno != errno.ENOENT or error.read_class != f"cgroup-v2-depth-0-{name}-open":
                            raise
                    else:
                        raise ObservationError("validation", None, "root scalar controls imply incomplete ancestry")
                records.append({"depth": depth, "limit": None, "current": None,
                                "root": "positive memory.stat and exact ENOENT scalar controls"})
            else:
                maximum = control(descriptor, "memory.max", depth)
                current = control(descriptor, "memory.current", depth)
                require(maximum == "max" or maximum.isdigit(), "invalid memory.max")
                require(current.isdigit(), "invalid memory.current")
                records.append({"depth": depth, "limit": None if maximum == "max" else int(maximum), "current": int(current)})
            after = record_read(f"cgroup-v2-depth-{depth}-directory-stat-after", lambda: os.fstat(descriptor))
            require((before.st_dev, before.st_ino) == (after.st_dev, after.st_ino), "cgroup changed during observation")
            if depth < len(components):
                child = record_read(f"cgroup-v2-descend-{depth + 1}", lambda: os.open(components[depth], os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor))
                os.close(descriptor)
                descriptor = child
    finally:
        os.close(descriptor)
    require(record_read("proc-self-cgroup-recheck", lambda: Path("/proc/self/cgroup").read_text()) == cgroup, "process migrated")
    require(record_read("proc-self-mountinfo-recheck", lambda: Path("/proc/self/mountinfo").read_text()) == mounts, "mounts changed")
    require(record_read("proc-self-namespace-recheck", lambda: os.readlink("/proc/self/ns/cgroup")) == namespace, "namespace changed")
    finite = [item for item in records if item["limit"] is not None]
    available = min([fields["MemAvailable"]] + [max(0, item["limit"] - item["current"]) for item in finite])
    capacity = min([fields["MemTotal"]] + [item["limit"] for item in finite])
    return {"MemTotal": fields["MemTotal"], "MemAvailable": fields["MemAvailable"],
            "effective_capacity_bytes": capacity, "effective_headroom_bytes": available,
            "visible_cgroups": [{"depth": item["depth"], "cap_bytes": item["limit"],
                                  "current_bytes": item["current"],
                                  **({"root_exception": item["root"]} if "root" in item else {})}
                                 for item in records]}
