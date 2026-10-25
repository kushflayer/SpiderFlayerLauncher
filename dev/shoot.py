import subprocess, time, sys
from playwright.sync_api import sync_playwright
srv = subprocess.Popen(["python3", "-m", "http.server", "8765", "-d", "/home/claude/spiderflayer/ui"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1)
errors = []
out = "/tmp/shots/"
import os; os.makedirs(out, exist_ok=True)
try:
    with sync_playwright() as p:
        b = p.chromium.launch()
        ctx = b.new_context(viewport={"width": 1180, "height": 740})
        ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        pg = ctx.new_page()
        pg.on("console", lambda m: errors.append(f"console.{m.type}: {m.text}") if m.type in ("error", "warning") else None)
        pg.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
        pg.goto("http://localhost:8765/index.html"); pg.wait_for_timeout(700)
        pg.screenshot(path=out + "1-home.png")
        pg.click('[data-to="library"]'); pg.wait_for_timeout(200); pg.screenshot(path=out + "2-library.png")
        pg.click('.card[data-act="open-inst"] >> nth=0'); pg.wait_for_timeout(300); pg.screenshot(path=out + "3-instance.png")
        pg.click('[data-tab="options"]'); pg.wait_for_timeout(150); pg.screenshot(path=out + "3b-options.png")
        pg.click('[data-to="browse"]'); pg.wait_for_timeout(500); pg.screenshot(path=out + "4-browse.png")
        pg.click('[data-to="settings"]'); pg.wait_for_timeout(250); pg.screenshot(path=out + "5-settings.png")
        pg.click('[data-act="create"] >> nth=0'); pg.wait_for_timeout(500); pg.screenshot(path=out + "6-create.png")
        pg.keyboard.press("Escape")
        pg.click('button[data-act="accounts"] >> nth=0'); pg.wait_for_timeout(200); pg.screenshot(path=out + "7-accounts.png")
        b.close()
finally:
    srv.terminate()
print("\n".join(errors) if errors else "NO CONSOLE ERRORS")
