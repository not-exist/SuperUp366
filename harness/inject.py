import os, sys, subprocess, tempfile, shutil

ROOT = r'H:\project\up366'
U3ENC = os.path.join(ROOT, 'tools', 'u3enc.js')
PAYLOAD_DEV = os.path.join(ROOT, 'harness', 'payload', '_dev')
TAG = '<script src="./_dev/hook.js"></script>'

def run(mode, src, dst):
    r = subprocess.run(['node', U3ENC, mode, src, dst], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip() or r.stdout.strip())

def main(res_dir):
    n = 0
    for dirpath, _, files in os.walk(res_dir):
        for fn in files:
            if not fn.endswith('.html.u3enc'):
                continue
            path = os.path.join(dirpath, fn)
            tmp_in = os.path.join(tempfile.gettempdir(), 'u3_in.html')
            run('decrypt', path, tmp_in)
            html = open(tmp_in, 'r', encoding='utf-8', errors='replace').read()
            if TAG in html:
                shutil.copy(os.path.join(PAYLOAD_DEV, 'hook.js'), os.path.join(dirpath, '_dev', 'hook.js'))
                print('skip (already injected):', os.path.relpath(path, res_dir))
                continue
            if '</body>' in html:
                html = html.replace('</body>', TAG + '\n</body>', 1)
            else:
                html = html + '\n' + TAG
            open(tmp_in, 'w', encoding='utf-8', newline='').write(html)
            run('encrypt', tmp_in, path)
            dev = os.path.join(dirpath, '_dev')
            os.makedirs(dev, exist_ok=True)
            shutil.copy(os.path.join(PAYLOAD_DEV, 'hook.js'), os.path.join(dev, 'hook.js'))
            print('injected:', os.path.relpath(path, res_dir))
            n += 1
    print('total injected:', n)

if __name__ == '__main__':
    main(sys.argv[1])
