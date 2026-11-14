"""Embed the Rust game in the offline player."""
import base64
from pathlib import Path
root=Path(__file__).resolve().parents[1]
wasm=root/'target/wasm32v1-none/release/night_post_web.wasm'
template=(root/'web/player.html').read_text()
(root/'dist').mkdir(exist_ok=True)
(root/'dist/Night_Post.html').write_text(template.replace('__WASM__',base64.b64encode(wasm.read_bytes()).decode()))
