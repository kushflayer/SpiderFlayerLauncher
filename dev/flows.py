import subprocess, time, base64, io
from playwright.sync_api import sync_playwright
from PIL import Image
srv = subprocess.Popen(["python3","-m","http.server","8766","-d","/home/claude/spiderflayer/ui"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
time.sleep(1)
errors=[]; results=[]
def check(name, cond):
    results.append((name, bool(cond))); print(("PASS " if cond else "FAIL ")+name)
try:
    with sync_playwright() as p:
        b=p.chromium.launch()
        ctx=b.new_context(viewport={"width":1180,"height":740})
        ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        pg=ctx.new_page()
        pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.on("console", lambda m: errors.append(m.text) if m.type=="error" and "TUNNEL" not in m.text else None)
        # first run: no account -> modal appears
        pg.goto("http://localhost:8766/index.html?noacct"); pg.wait_for_timeout(600)
        check("first run opens accounts modal", pg.locator(".modal h2:has-text('Accounts')").count()==1)
        pg.fill("#off-name","ab"); pg.click('[data-act="acct-offline"]'); pg.wait_for_timeout(200)
        check("short name shows error toast", pg.locator(".toast.error").count()>=1)
        pg.fill("#off-name","Steve_01"); pg.click('[data-act="acct-offline"]'); pg.wait_for_timeout(200)
        check("offline account added + active", pg.locator(".acct.sel:has-text('Steve_01')").count()==1)
        check("MS sign-in is one button", pg.locator('[data-act="ms-login"]').count()==1)
        pg.click('[data-act="close-modal"]')
        # create fabric instance
        pg.click('.rail-btn[data-act="create"]'); pg.wait_for_timeout(400)
        pg.select_option("#c-loader","fabric"); pg.wait_for_timeout(300)
        check("loader version list filled", pg.locator("#c-lv option").count()==3)
        pg.select_option("#c-mc","1.12.2"); pg.fill("#c-name","Test Pack"); pg.wait_for_timeout(300)
        pg.click("#c-go"); pg.wait_for_timeout(400)
        check("create -> lands on instance page", pg.locator("h1:has-text('Test Pack')").count()==1)
        # unsupported loader/version disables create
        pg.click('.rail-btn[data-act="create"]'); pg.wait_for_timeout(300)
        pg.check("#c-snap"); pg.select_option("#c-mc","1.8.9"); pg.select_option("#c-loader","fabric"); pg.wait_for_timeout(300)
        check("unsupported loader version disables Create", pg.locator("#c-go").is_disabled())
        pg.keyboard.press("Escape")
        # add content -> browse locked to instance, install
        pg.click('.inst-head [data-act="add-content"]'); pg.wait_for_timeout(500)
        check("browse target preselected & version locked", pg.locator("#f-mc").is_disabled() and pg.input_value("#f-target")=="test-pack")
        pg.click('[data-act="install"] >> nth=0'); pg.wait_for_timeout(400)
        check("install shows success toast", pg.locator(".toast.ok:has-text('installed')").count()>=1)
        # search filter
        pg.fill("#q","sodi"); pg.wait_for_timeout(700)
        check("search narrows results", pg.locator(".proj").count()==1)
        # modpack tab creates instance
        pg.click('[data-act="browse-kind"][data-k="modpack"]'); pg.wait_for_timeout(500)
        pg.fill("#q",""); pg.wait_for_timeout(600)
        pg.click('[data-act="open-versions"] >> nth=0'); pg.wait_for_timeout(600)
        pg.click('[data-act="ver-install"] >> nth=0'); pg.wait_for_timeout(600)
        check("modpack install creates + opens instance", pg.locator("h1:has-text('Pack v5')").count()==1)
        # play
        pg.click('.inst-head .js-play'); pg.wait_for_timeout(200)
        check("progress card shown while preparing", not pg.locator("#progress").is_hidden())
        pg.wait_for_timeout(500)
        check("button flips to Stop when running", "Stop" in pg.inner_text(".inst-head .js-play"))
        check("top bar shows running state", "running" in pg.inner_text("#runstate"))
        pg.click('[data-tab="logs"]'); pg.wait_for_timeout(200)
        check("logs tab has game output", "Setting user" in pg.inner_text("#logbox"))
        pg.evaluate("window.__emit('log',{id:document.querySelector('.js-play').dataset.id,line:'LIVE LINE'})")
        check("live log lines append", "LIVE LINE" in pg.inner_text("#logbox"))
        pg.evaluate("window.__emit('game-exit',{id:document.querySelector('.js-play').dataset.id,code:1})"); pg.wait_for_timeout(200)
        check("crash exit code toasts + button resets", pg.locator(".toast.error:has-text('exit code 1')").count()>=1 and "Play" in pg.inner_text(".inst-head .js-play"))
        # content toggle
        pg.click('[data-to="library"]'); pg.wait_for_timeout(200)
        pg.click('.card[data-act="open-inst"]:has-text("Vanilla+ Fabric")'); pg.wait_for_timeout(300)
        pg.click('[data-act="toggle-file"] >> nth=0'); pg.wait_for_timeout(300)
        check("toggling a mod disables it", pg.locator(".file-row.off").count()==2)
        # options save + delete
        pg.click('[data-tab="options"]'); pg.fill("#o-name","Renamed"); pg.click('[data-act="save-inst"]'); pg.wait_for_timeout(300)
        check("rename saved", pg.locator("h1:has-text('Renamed')").count()==1)
        pg.click('[data-tab="options"]'); pg.click('[data-act="delete-inst"]'); pg.click("#del-go"); pg.wait_for_timeout(400)
        check("delete returns to library without it", pg.locator(".card:has-text('Renamed')").count()==0)
        # settings: theme, accent, bg
        pg.click('[data-to="settings"]'); pg.wait_for_timeout(200)
        pg.click('[data-act="theme"][data-t="dark"]'); pg.wait_for_timeout(150)
        check("dark theme applied", pg.evaluate("document.documentElement.dataset.theme")=="dark")
        pg.click('[data-act="theme"][data-t="oled"]')
        pg.click('[data-act="accent"][data-c="#ec4899"]'); pg.wait_for_timeout(150)
        check("accent applied", pg.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()")=="#ec4899")
        buf=io.BytesIO(); Image.new("RGB",(64,64),(10,200,90)).save(buf,"PNG"); open("/tmp/test-bg.png","wb").write(buf.getvalue())
        pg.set_input_files("#bg-file","/tmp/test-bg.png"); pg.wait_for_timeout(400)
        calls=[c[0] for c in pg.evaluate("window.__calls")]
        check("custom background uploaded via backend", "set_background" in calls)
        check("reset button now enabled", not pg.locator('[data-act="reset-bg"]').is_disabled())
        pg.click('[data-act="reset-bg"]'); pg.wait_for_timeout(300)
        check("reset returns to default background", "default-bg.webp" in pg.evaluate("document.getElementById('bg').style.backgroundImage"))
        # slider persists
        pg.evaluate("(()=>{const e=document.querySelector('[data-slide=glass]');e.value=20;e.dispatchEvent(new Event('input'))})()"); pg.wait_for_timeout(600)
        saved=[c for c in pg.evaluate("window.__calls") if c[0]=="save_settings"]
        check("glass slider saved to backend", saved and saved[-1][1]["settings"]["glass"]==20)
        pg.screenshot(path="/tmp/shots/8-settings-after.png")
        b.close()
finally:
    srv.terminate()
print("\nJS ERRORS:", errors if errors else "none")
print(f"{sum(ok for _,ok in results)}/{len(results)} passed")
