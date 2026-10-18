"""Controlled slow renderer for exit/cleanup checks; never produces a real video."""
import hashlib
import sys
import time
import uuid
from pathlib import Path
from test_render_jobs import call


def environment(root):
    font = root/'font'; font.write_bytes(b'exit fixture font')
    worker = root/'worker.py'
    worker.write_text("#!" + sys.executable + "\nfrom pathlib import Path\nimport time, os\nPath('frame-999.png').write_bytes(b'partial')\nPath('worker-started').write_text(str(os.getpid()))\ntime.sleep(300)\n")
    worker.chmod(0o700)
    return dict(SUNGEKI_RENDER_FFMPEG=str(worker), SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
                SUNGEKI_RENDER_FONT=str(font), SUNGEKI_RENDER_FONT_SHA256=hashlib.sha256(font.read_bytes()).hexdigest(),
                SUNGEKI_RENDER_WORK_ROOT=str(root/'render'))


def start(client, root):
    job=str(uuid.uuid4())
    snap=call(client,'create_project',title='終了確認専用・停止待ちの試験作品 '+job[:8])['snapshot']
    snap=call(client,'apply_commands',operation_id=str(uuid.uuid4()),commands=[dict(type='add_scene',
        scene=dict(id=str(uuid.uuid4()),name='終了確認',start_us=0,duration_us=100000))])['snapshot']
    saved=call(client,'save_project',path=str(root/f'終了確認専用-{job}.vvm.json'),expected_revision=snap['revision'])
    assert not saved.get('isError'), saved
    result=call(client,'start_render_job',job_id=job,session_id=snap['session_id'],expected_revision=snap['revision'])
    assert not result.get('isError'), result
    marker=root/'render'/job/'worker-started'
    deadline=time.monotonic()+10
    while not marker.exists() and time.monotonic()<deadline: time.sleep(.02)
    assert marker.exists(), result
    return job, marker
