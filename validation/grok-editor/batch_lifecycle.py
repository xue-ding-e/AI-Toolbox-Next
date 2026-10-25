"""Bounded synthetic-fixture checkpoints; no process launch or signaling here.

The still-live outer wrapper is the ownership root. A reaped batch PID is never
used as a session/group leader or signal target. This observes ordinary owned
descendants under the existing hosted-runner boundary, not hostile same-UID
containment. Node reports child close; Python independently checks the previously
observed generation is absent and the current owned tree contains only wrapper.
"""
import errno
import json
import os
import re
import stat

LIMIT = 1024
SAFE_INTEGER = 2**53 - 1


class LifecycleError(RuntimeError):
    def __init__(self, read_class, errno_value=None):
        self.read_class, self.errno = read_class, errno_value
        super().__init__(f'{read_class}; errno={errno_value}')


def require(value, label):
    if not value:
        raise LifecycleError(label)


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'batch-control-duplicate-key')
        result[key] = value
    return result


def signature(st):
    return (st.st_dev, st.st_ino, st.st_mode, st.st_uid, st.st_size, st.st_mtime_ns, st.st_ctime_ns)


def valid_identity(identity):
    return all(type(value) is int and 0 < value <= SAFE_INTEGER for value in identity)


class BatchLifecycle:
    def __init__(self, directory, session, owned_snapshot, process_snapshot, record):
        require(os.path.isabs(directory) and os.path.normpath(directory) == os.fspath(directory),
                'batch-control-directory-path')
        require(re.fullmatch(r'[0-9a-f]{32}', session) is not None, 'batch-session-invalid')
        self.directory, self.session = directory, session
        self.owned_snapshot, self.process_snapshot, self.record = owned_snapshot, process_snapshot, record
        self.fd = None
        self.index, self.active, self.wrapper = 0, None, None
        self.records, self.seals = [], {}
        try:
            self.fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            st = os.fstat(self.fd)
            require(stat.S_ISDIR(st.st_mode) and stat.S_IMODE(st.st_mode) == 0o700
                    and st.st_uid == os.getuid(), 'batch-control-directory-mode-owner')
            self.directory_id = (st.st_dev, st.st_ino)
            self.check_directory()
            with os.scandir(self.fd) as entries:
                require(next(entries, None) is None, 'batch-control-directory-not-empty')
            self.record([])
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None

    def check_directory(self):
        try:
            held, current = os.fstat(self.fd), os.stat(self.directory, follow_symlinks=False)
            require(stat.S_ISDIR(current.st_mode) and stat.S_IMODE(current.st_mode) == 0o700
                    and current.st_uid == os.getuid(), 'batch-control-directory-mode-owner')
            require((held.st_dev, held.st_ino) == self.directory_id == (current.st_dev, current.st_ino),
                    'batch-control-directory-identity')
        except OSError as error:
            raise LifecycleError('batch-control-directory-observation', error.errno) from None

    def names(self):
        self.check_directory()
        try:
            allowed = set(self.seals)
            if self.index < 2:
                allowed |= {f'request-{self.index}.json', f'.request-{self.index}.tmp'}
            names = set()
            # Stop at the first unexpected name; never allocate an unbounded
            # directory listing before enforcing the finite protocol inventory.
            with os.scandir(self.fd) as entries:
                for entry in entries:
                    require(entry.name in allowed and len(names) < len(allowed),
                            'batch-control-unexpected-entry-or-order')
                    names.add(entry.name)
            for name in list(names):
                try:
                    st = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
                except OSError as error:
                    # The one current staging alias is intentionally unlinked
                    # by its producer. Committed records disappearing are not
                    # this finite publication transition and still fail closed.
                    if error.errno == errno.ENOENT and self.index < 2 and name == f'.request-{self.index}.tmp':
                        names.remove(name)
                        continue
                    raise
                require(stat.S_ISREG(st.st_mode) and st.st_size <= LIMIT
                        and stat.S_IMODE(st.st_mode) == 0o600 and st.st_uid == os.getuid(),
                        'batch-control-entry-type-or-size')
            return names
        except OSError as error:
            raise LifecycleError('batch-control-entry-observation', error.errno) from None

    def read(self, name, missing=False, staging=None):
        self.check_directory()
        fd = None
        try:
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=self.fd)
            if staging is not None:
                # Check AFTER opening the committed name: checking before open
                # leaves a window for link/unlink to alter ctime during reading.
                try:
                    pending = os.stat(staging, dir_fd=self.fd, follow_symlinks=False)
                except OSError as error:
                    if error.errno != errno.ENOENT:
                        raise
                else:
                    require(stat.S_ISREG(pending.st_mode) and pending.st_size <= LIMIT
                            and stat.S_IMODE(pending.st_mode) == 0o600 and pending.st_uid == os.getuid(),
                            'batch-control-entry-type-or-size')
                    return None
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_size <= LIMIT
                    and stat.S_IMODE(before.st_mode) == 0o600 and before.st_uid == os.getuid(),
                    'batch-control-entry-type-or-size')
            data = os.read(fd, LIMIT + 1)
            require(len(data) <= LIMIT, 'batch-control-entry-type-or-size')
            after = os.fstat(fd)
            current = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
            require(signature(before) == signature(after) == signature(current)
                    and len(data) == before.st_size, 'batch-control-file-identity')
            self.check_directory()
            return data, signature(before)
        except OSError as error:
            # Only the committed-name open can mean "not published yet".
            # After opening, disappearance of that committed name is an error.
            if missing and fd is None and error.errno == errno.ENOENT:
                return None
            raise LifecycleError('batch-control-read', error.errno) from None
        finally:
            if fd is not None:
                os.close(fd)

    def publish_ack(self, request, start):
        ack = {**request, 'startTicks': start, 'status': 'quiescent'}
        raw = canonical(ack)
        temporary, final = f'.ack-{self.index}.tmp', f'ack-{self.index}.json'
        self.check_directory()
        fd = None
        try:
            fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o600, dir_fd=self.fd)
            require(os.write(fd, raw) == len(raw), 'batch-control-short-write')
            os.close(fd); fd = None
            os.link(temporary, final, src_dir_fd=self.fd, dst_dir_fd=self.fd, follow_symlinks=False)
            os.unlink(temporary, dir_fd=self.fd)
            self.seals[final] = self.read(final)
            require(self.seals[final][0] == raw, 'batch-control-ack-content-changed')
        except OSError as error:
            raise LifecycleError('batch-control-ack-publication', error.errno) from None
        finally:
            if fd is not None:
                os.close(fd)

    def observe(self, child, known, members):
        names = self.names()
        if self.index == 2:
            return  # Final mandatory check still rejects mutated/missing records.
        leader = members.get(child.pid)
        require(leader is not None and known.get(child.pid) == leader['start'], 'batch-wrapper-identity-unknown')
        identity = (child.pid, leader['start'])
        require(valid_identity(identity), 'batch-wrapper-generation-invalid')
        if self.wrapper is None:
            self.wrapper = identity
        require(identity == self.wrapper, 'batch-wrapper-generation-changed')
        direct = [(pid, item['start']) for pid, item in members.items()
                  if pid != child.pid and item['ppid'] == child.pid]
        if self.active is None and direct:
            require(len(direct) == 1, 'batch-multiple-direct-children')
            self.active = direct[0]
            require(valid_identity(self.active) and known.get(self.active[0]) == self.active[1],
                    'batch-child-generation-unknown')
        elif self.active is not None:
            require(all(item == self.active for item in direct), 'batch-overlap-or-generation-change')

        name = f'request-{self.index}.json'
        # The Node writer unlinks its staging hardlink after publication. Until
        # then its ctime can legitimately change: do not read or ACK partial
        # publication. This finite IPC state is still inside existing deadlines;
        # observation errors are never converted into "not ready".
        if f'.request-{self.index}.tmp' in names:
            return
        observed = self.read(name, missing=True, staging=f'.request-{self.index}.tmp')
        if observed is None:
            return
        raw, file_id = observed
        try:
            request = json.loads(raw, object_pairs_hook=unique_object)
        except (ValueError, UnicodeError):
            raise LifecycleError('batch-control-invalid-json') from None
        require(type(request) is dict and set(request) == {'schema', 'session', 'index', 'pid', 'exitCode'},
                'batch-control-schema')
        require(canonical(request) == raw, 'batch-control-noncanonical-json')
        require(type(request['schema']) is int and request['schema'] == 1
                and request['session'] == self.session
                and type(request['index']) is int and request['index'] == self.index,
                'batch-control-session-or-order')
        require(type(request['pid']) is int and 0 < request['pid'] <= SAFE_INTEGER
                and type(request['exitCode']) is int and 0 <= request['exitCode'] <= 255,
                'batch-control-pid-or-exit')
        require(self.active is not None and request['pid'] == self.active[0]
                and known.get(self.active[0]) == self.active[1], 'batch-child-never-observed-or-unknown')

        # Re-read after seeing the checkpoint; a pre-checkpoint members snapshot
        # is not proof of quiescence. Keep the live wrapper as the sole scan root.
        try:
            fresh = self.owned_snapshot(child.pid, known)
            table = self.process_snapshot()
        except OSError as error:
            raise LifecycleError('batch-owned-observation', error.errno) from None
        require(set(fresh) == {child.pid} and fresh[child.pid]['start'] == self.wrapper[1],
                'batch-owned-descendants-remain-or-wrapper-unknown')
        require(self.active[0] not in table, 'batch-child-still-present-or-pid-reused')
        require(self.read(name) == (raw, file_id), 'batch-control-request-changed')
        entry = {'index': self.index, 'pid': self.active[0], 'startTicks': self.active[1],
                 'exitCode': request['exitCode'], 'wrapperPid': child.pid,
                 'wrapperStartTicks': self.wrapper[1], 'ownedRemaining': [child.pid],
                 'status': 'quiescence-observed'}
        self.records.append(entry)
        self.record(self.records)
        self.publish_ack(request, self.active[1])
        self.seals[name] = (raw, file_id)
        entry['status'] = 'ack-published'
        self.record(self.records)
        self.index += 1
        self.active = None

    def assert_complete(self):
        require(self.index == 2 and len(self.records) == 2, 'batch-handshake-incomplete')
        require(self.names() == set(self.seals) == {'request-0.json', 'ack-0.json', 'request-1.json', 'ack-1.json'},
                'batch-control-final-inventory')
        require(all(self.read(name) == seal for name, seal in self.seals.items()), 'batch-control-final-seal')
        require(all(record['exitCode'] == 0 for record in self.records), 'batch-nonzero-exit')
