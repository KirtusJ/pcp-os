"""SQLite and OS-process surrogate only. This does NOT run compiled Keyrook.

Exercise staging/commit/hard-link publication under SIGKILL. All inputs are
synthetic; no production data, cryptographic keys or PCP OS kernel is used.
"""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time

CHILD = r'''
import os, sqlite3, sys, time
from pathlib import Path
p=Path(sys.argv[1]); phase=sys.argv[2]
s=p.parent/f'.{p.name}.keyrook-stage-{os.getpid()}.sqlite'
fd=os.open(s,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.close(fd)
if phase=='reserved': print('RESERVED',flush=True);time.sleep(100)
with sqlite3.connect(s) as db:
 db.execute('PRAGMA journal_mode=DELETE')
 db.execute('PRAGMA synchronous=FULL')
 db.execute('BEGIN IMMEDIATE')
 db.execute('CREATE TABLE episodes(bits INT, seed INT)')
 db.execute('INSERT INTO episodes VALUES(8,31415)')
 if phase=='uncommitted': print('UNCOMMITTED',flush=True);time.sleep(100)
 db.commit()
with open(s,'rb') as fd: os.fsync(fd.fileno())
if phase=='committed': print('COMMITTED',flush=True);time.sleep(100)
os.link(s,p)
if phase=='published': print('PUBLISHED',flush=True);time.sleep(100)
os.unlink(s)
'''


def publish(target):
    stage = target.with_name(f'.{target.name}.keyrook-stage-retry-{os.getpid()}-{time.time_ns()}.sqlite')
    fd = os.open(stage, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(fd)
    try:
        with sqlite3.connect(stage) as db:
            db.execute('PRAGMA journal_mode=DELETE')
            db.execute('PRAGMA synchronous=FULL')
            db.execute('BEGIN IMMEDIATE')
            db.execute('CREATE TABLE episodes(bits INT, seed INT)')
            db.execute('INSERT INTO episodes VALUES(8,31415)')
            db.commit()
        with open(stage,'rb') as source:
            os.fsync(source.fileno())
        os.link(stage,target)
    finally:
        stage.unlink(missing_ok=True)


def check(target):
    with sqlite3.connect(target) as db:
        assert db.execute('PRAGMA integrity_check').fetchone() == ('ok',)
        assert db.execute('SELECT bits,seed FROM episodes').fetchall() == [(8,31415)]
    return hashlib.sha256(target.read_bytes()).hexdigest()


def main():
    outcomes=[]
    with tempfile.TemporaryDirectory(prefix='keyrook-publish-surrogate-') as dirname:
        directory=Path(dirname)
        for phase in ('reserved','uncommitted','committed','published'):
            target=directory/(phase+'.sqlite')
            child=subprocess.Popen([sys.executable,'-u','-c',CHILD,str(target),phase],
                                   stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            expected=phase.upper()
            line=child.stdout.readline().strip()
            assert line==expected,(phase,line,child.stderr.read())
            child.kill()
            child.wait(timeout=5)
            assert child.returncode < 0
            existed=target.exists()
            assert existed == (phase=='published'),phase
            before=check(target) if existed else None
            if existed:
                try: publish(target)
                except FileExistsError: pass
                else: raise AssertionError('published destination overwritten')
                assert check(target)==before
                result='preserved-published'
            else:
                publish(target)
                check(target)
                result='retried-successfully'
            orphans=list(directory.glob(f'.{target.name}.keyrook-stage-*'))
            assert len(orphans)>=1
            outcomes.append({'stop':phase,'kill_exit':child.returncode,
                             'target_before_retry':existed,'retry':result,
                             'stale_stage_files':len(orphans)})
        protected=directory/'protected.sqlite'
        protected.write_bytes(b'pre-existing contents')
        try: publish(protected)
        except FileExistsError: pass
        else: raise AssertionError('existing target overwritten')
        assert protected.read_bytes()==b'pre-existing contents'
    print(json.dumps({'runtime':'Python '+sys.version.split()[0],
                      'sqlite':sqlite3.sqlite_version,'target':'staging/SQLite/hardlink surrogate',
                      'not':'Keyrook Rust binary', 'killed_stages':outcomes,
                      'existing_destination_preserved':True,'result':'PASS'},sort_keys=True))

if __name__=='__main__':main()
