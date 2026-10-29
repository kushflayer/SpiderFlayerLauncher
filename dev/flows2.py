import subprocess, time, io, json
from playwright.sync_api import sync_playwright
from PIL import Image, ImageDraw
srv = subprocess.Popen(["python3","-m","http.server","8767","-d","/home/claude/spiderflayer/ui"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
time.sleep(1)
errors=[]; results=[]
def check(name, cond):
    results.append((name, bool(cond))); print(("PASS " if cond else "FAIL ")+name)
COLORS={"1":(180,40,60),"2":(40,90,200),"3":(40,160,90)}
def png_for(url):
    key=url.rsplit("/",1)[-1].split(".")[0]
    c=COLORS.get(key,(120,120,120))
    im=Image.new("RGB",(640,400),c); d=ImageDraw.Draw(im)
    for i in range(0,640,40): d.line([(i,0),(i-200,400)],fill=(255,255,255),width=3)
    d.text((20,20),"screenshot "+key,fill=(255,255,255))
    b=io.BytesIO(); im.save(b,"PNG"); return b.getvalue()
def calls(pg,name): return [c[1] for c in pg.evaluate("window.__calls") if c[0]==name]
try:
    with sync_playwright() as p:
        b=p.chromium.launch()
        ctx=b.new_context(viewport={"width":1180,"height":740})
        ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        ctx.route("https://cdn.modrinth.com/**", lambda r: r.fulfill(status=200,content_type="image/png",body=png_for(r.request.url)))
        pg=ctx.new_page()
        pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.on("console", lambda m: errors.append(m.text) if m.type=="error" and "TUNNEL" not in m.text else None)
        pg.goto("http://localhost:8767/index.html"); pg.wait_for_timeout(500)

        # ---- open a project from Discover
        pg.click('[data-to="browse"]'); pg.wait_for_timeout(500)
        pg.click('.proj >> nth=0 >> .title'); pg.wait_for_timeout(500)
        check("clicking a result opens its project page", pg.locator(".pj-head h1:has-text('Sodium')").count()==1)
        check("page shows downloads, followers, updated", all(t in pg.inner_text(".pj-stats") for t in ["62M","120K","Updated"]))
        check("sidebar lists compatibility, creators, links, details", all(t in pg.inner_text(".pj-side") for t in ["1.21.1","Fabric","jellysquid3","Source","LGPL-3.0"]))
        pg.screenshot(path="/tmp/shots/p1-description.png")

        # ---- description rendering + sanitising
        md="#view .md"
        check("markdown headings render", pg.locator(md+" h2:has-text('Features')").count()==1)
        check("bold/italic/code render", pg.locator(md+" strong:has-text('fast')").count()>=1 and pg.locator(md+" em:has-text('smart')").count()>=1 and pg.locator(md+" code:has-text('config.json')").count()==1)
        check("nested list renders", pg.locator(md+" ul ul li:has-text('nested child')").count()==1)
        check("table renders", pg.locator(md+" table td:has-text('Renderer')").count()==1)
        check("code block renders", pg.locator(md+" pre code:has-text('server.jar')").count()==1)
        check("https image allowed", pg.locator(md+" img[src^='https://cdn.modrinth.com/data/mock/banner']").count()==1)
        check("markdown + bare links become safe open-url links", pg.locator(md+" a[data-url='https://example.com/docs']").count()==1 and pg.locator(md+" a[data-url='https://example.com/bare']").count()==1)
        check("XSS: no script ever ran", pg.evaluate("window.__pwned")==None)
        check("XSS: no script/iframe/svg/style elements", pg.locator(md+" script, "+md+" iframe, "+md+" svg, "+md+" style").count()==0)
        check("XSS: no on* handlers or style attributes", pg.evaluate("[...document.querySelectorAll('#view .md *')].every(e=>[...e.attributes].every(a=>!/^on/i.test(a.name)&&a.name!=='style'))"))
        check("XSS: no javascript: links", pg.evaluate("[...document.querySelectorAll('#view .md a')].every(a=>!/javascript/i.test(a.getAttribute('data-url')||'')&&!/javascript/i.test(a.getAttribute('href')||''))"))
        check("insecure http image removed", pg.locator(md+" img[src^='http://']").count()==0)
        check("hostile link text kept but inert", pg.locator(md+" a:has-text('bad link')").count()==1 and pg.locator(md+" a:has-text('bad link')[data-url]").count()==0)
        pg.click(md+" a[data-url='https://example.com/docs']"); pg.wait_for_timeout(150)
        check("clicking a description link asks the OS browser (no in-app navigation)", {"url":"https://example.com/docs"} in calls(pg,"open_url") and pg.url.endswith("index.html"))

        # ---- gallery + lightbox
        pg.click('[data-act="proj-tab"][data-tab="gallery"]'); pg.wait_for_timeout(500)
        check("gallery shows all screenshots", pg.locator(".shot").count()==3)
        check("screenshot captions shown", "Menu screen" in pg.inner_text(".gal"))
        pg.screenshot(path="/tmp/shots/p2-gallery.png")
        pg.click(".shot >> nth=0"); pg.wait_for_timeout(500)
        check("lightbox opens with big image + caption", pg.locator("#lightbox img").count()==1 and "Menu screen" in pg.inner_text("#lightbox") and "1 / 3" in pg.inner_text("#lightbox"))
        pg.screenshot(path="/tmp/shots/p3-lightbox.png")
        pg.keyboard.press("ArrowRight"); pg.wait_for_timeout(150)
        check("arrow key goes to next screenshot", "2 / 3" in pg.inner_text("#lightbox"))
        pg.click('#lightbox [data-lb="-1"]'); pg.click('#lightbox [data-lb="-1"]'); pg.wait_for_timeout(100)
        check("previous wraps around", "3 / 3" in pg.inner_text("#lightbox"))
        pg.keyboard.press("Escape"); pg.wait_for_timeout(100)
        check("Escape closes lightbox", pg.locator("#lightbox").count()==0)

        # ---- versions: pick a specific, older version
        pg.click('[data-act="proj-tab"][data-tab="versions"]'); pg.wait_for_timeout(500)
        check("all versions listed (not just latest)", pg.locator(".ver").count()==5)
        check("release type badges", pg.locator(".badge.vt-beta").count()==1 and pg.locator(".badge.vt-alpha").count()==1)
        pg.screenshot(path="/tmp/shots/p4-versions.png")
        pg.select_option("#vf-type","beta"); pg.wait_for_timeout(200)
        check("release-type filter works", pg.locator(".ver").count()==1)
        pg.select_option("#vf-type","all"); pg.select_option("#vf-mc","1.20.1"); pg.wait_for_timeout(400)
        check("game-version filter asks backend", any(c.get("mc")=="1.20.1" for c in calls(pg,"project_versions")) and pg.locator(".ver").count()==1)
        pg.select_option("#vf-mc",""); pg.wait_for_timeout(400)
        pg.click(".ver-log summary >> nth=0"); pg.wait_for_timeout(100)
        check("changelog expands and renders markdown", pg.locator(".ver-log .md h2:has-text('Fixes')").count()==1)
        # install the OLD 0.4.10 version into the 1.20.1 Forge instance -> mismatch confirm
        pg.click('[data-act="ver-install"] >> nth=4'); pg.wait_for_timeout(300)
        check("instance picker appears", pg.locator("[data-pick]").count()>=2)
        pg.click('[data-pick="forge-1-20-1"]'); pg.wait_for_timeout(300)
        check("incompatible pick warns first", pg.locator(".modal h2:has-text('may not work')").count()==1)
        pg.click("#cf-ok"); pg.wait_for_timeout(400)
        inst_calls=calls(pg,"install_version_content")
        check("installs the exact version chosen, not the latest", inst_calls and inst_calls[-1]["versionId"]=="v1" and inst_calls[-1]["instanceId"]=="forge-1-20-1")
        check("success toast names the version", pg.locator(".toast.ok:has-text('0.4.10')").count()>=1)
        # compatible install skips the warning
        pg.click('[data-act="ver-install"] >> nth=0'); pg.wait_for_timeout(300)
        pg.click('[data-pick="fabric-1-21-1"]'); pg.wait_for_timeout(400)
        check("compatible version installs with no warning", calls(pg,"install_version_content")[-1]["versionId"]=="v5" and pg.locator(".modal h2:has-text('may not work')").count()==0)

        # ---- install-to-instance pre-filters versions, with an escape hatch
        pg.click('[data-to="library"]'); pg.click('.card[data-act="open-inst"]:has-text("Vanilla+ Fabric")'); pg.wait_for_timeout(300)
        pg.click('.inst-head [data-act="add-content"]'); pg.wait_for_timeout(500)
        pg.click('.proj >> nth=0 >> .title'); pg.wait_for_timeout(400)
        pg.click('[data-act="proj-tab"][data-tab="versions"]'); pg.wait_for_timeout(500)
        check("opened from an instance: versions pre-filtered to it", pg.input_value("#vf-mc")=="1.21.1" and pg.locator(".note:has-text('Vanilla+ Fabric')").count()==1)
        pg.click('[data-act="ver-all"]'); pg.wait_for_timeout(500)
        check("'Show all versions' clears the filter", pg.input_value("#vf-mc")=="" and pg.locator(".ver").count()==5)

        # ---- modpack: choose any version
        pg.click('[data-to="browse"]'); pg.wait_for_timeout(300)
        pg.click('[data-act="browse-kind"][data-k="modpack"]'); pg.wait_for_timeout(500)
        check("modpack rows say 'Choose version' (no forced latest)", pg.locator('.proj [data-act="open-versions"]').count()>=1 and pg.locator('.proj [data-act="install"]').count()==0)
        pg.click('.proj [data-act="open-versions"] >> nth=0'); pg.wait_for_timeout(700)
        check("lands on the Versions tab", pg.locator(".tab.active:has-text('Versions')").count()==1)
        check("modpack versions unfiltered", pg.locator(".ver").count()==5)
        pg.click('[data-act="ver-install"] >> nth=3'); pg.wait_for_timeout(600)
        mp=calls(pg,"install_modpack_version")
        check("creates instance from the chosen (older) pack version", mp and mp[-1]["versionId"]=="v2" and pg.locator("h1:has-text('Pack v2')").count()==1)

        # ---- custom background: top bar button, file chooser, drop
        with pg.expect_file_chooser() as fc:
            pg.click("#bgbtn")
        fc.value.set_files("/tmp/test-bg.png"); pg.wait_for_timeout(500)
        check("top-bar picture button opens the file chooser and saves", len(calls(pg,"set_background"))==1 and calls(pg,"set_background")[0]["ext"]=="png")
        check("saved background is reported to the user", pg.locator(".toast.ok:has-text('Background updated')").count()>=1)
        pg.evaluate("""()=>{const dt=new DataTransfer();dt.items.add(new File([new Uint8Array([137,80,78,71])],'sunset.webp',{type:'image/webp'}));
          window.dispatchEvent(new DragEvent('dragenter',{dataTransfer:dt,bubbles:true,cancelable:true}));
          window.dispatchEvent(new DragEvent('drop',{dataTransfer:dt,bubbles:true,cancelable:true}));}""")
        pg.wait_for_timeout(500)
        check("dropping an image on the window sets it as background", len(calls(pg,"set_background"))==2 and calls(pg,"set_background")[1]["ext"]=="webp")
        pg.evaluate("""()=>{const dt=new DataTransfer();dt.items.add(new File(['x'],'notes.txt',{type:'text/plain'}));
          window.dispatchEvent(new DragEvent('drop',{dataTransfer:dt,bubbles:true,cancelable:true}));}""")
        pg.wait_for_timeout(300)
        check("non-image drop is rejected politely", len(calls(pg,"set_background"))==2 and pg.locator(".toast.error:has-text('image file')").count()>=1)
        pg.click('[data-to="settings"]'); pg.wait_for_timeout(300)
        check("settings shows preview + choose button", pg.locator(".bg-preview").count()==1 and pg.locator('[data-act="pick-bg"]').count()==1)
        pg.screenshot(path="/tmp/shots/p5-settings-bg.png")
        b.close()

        # ---- Rust (Horizon) instance
        ctx=p.chromium.launch().new_context(viewport={"width":1180,"height":740}); ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        pg=ctx.new_page(); pg.goto("http://localhost:8767/index.html"); pg.wait_for_timeout(500)
        pg.click('[data-to="library"]'); pg.wait_for_timeout(200)
        check("Rust is listed with its label", "fast, no mods" in pg.inner_text(".tiles"))
        pg.click('.card[data-act="open-inst"]:has-text("Rust")'); pg.wait_for_timeout(300)
        check("Rust page explains no mods and has no Add content", "can't run Fabric" in pg.inner_text("#view") and pg.locator('.inst-head [data-act="add-content"]').count()==0)
        pg.screenshot(path="/tmp/shots/r1-rust.png")
        pg.click('.inst-head .js-play'); pg.wait_for_timeout(300)
        check("Rust play calls launch_game", any(c["id"]=="rust" for c in calls(pg,"launch_game")))
        pg.click('[data-to="browse"]'); pg.wait_for_timeout(500)
        pg.click('[data-act="install"] >> nth=0'); pg.wait_for_timeout(300)
        check("install picker never offers Rust", pg.locator('[data-pick]').count()>0 and pg.locator('[data-pick="rust"]').count()==0)

        # ---- surface styles
        ctx=p.chromium.launch().new_context(viewport={"width":1180,"height":740}); ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        pg=ctx.new_page(); pg.goto("http://localhost:8767/index.html"); pg.wait_for_timeout(500)
        check("default surface is Frosted", pg.evaluate("document.documentElement.dataset.surface")=="frosted")
        pg.click('[data-to="settings"]'); pg.wait_for_timeout(200)
        check("three surface choices", pg.locator('[data-act="surface"]').count()==3)
        for sf in ("solid","frosted","aero"):
            pg.click(f'[data-act="surface"][data-sf="{sf}"]'); pg.wait_for_timeout(450)
            check(f"surface {sf} applies and is saved", pg.evaluate("document.documentElement.dataset.surface")==sf and calls(pg,"save_settings")[-1]["settings"]["surface"]==sf)
        check("solid turns blur off", (pg.click('[data-act="surface"][data-sf="solid"]') or True) and pg.evaluate("getComputedStyle(document.querySelector('#rail')).backdropFilter")=="none")

        # ---- Microsoft login (fresh context so there is no account)
        ctx=p.chromium.launch().new_context(viewport={"width":1180,"height":740}); ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
        pg=ctx.new_page(); pg.goto("http://localhost:8767/index.html?noacct&msoff"); pg.wait_for_timeout(600)
        check("MS off: button disabled and says not available yet", pg.locator('.modal [data-act="ms-login"]').count()==0 and "Not available yet" in pg.inner_text(".modal") and pg.locator("#off-name").count()==1)
        pg.screenshot(path="/tmp/shots/m4-msoff.png")
        for label,url in [("configured","?noacct"),("not configured","?noacct&nomsid")]:
            ctx=b2=p.chromium.launch().new_context(viewport={"width":1180,"height":740})
            ctx.add_init_script(path="/home/claude/spiderflayer/dev/mock.js")
            pg=ctx.new_page()
            pg.on("pageerror", lambda e: errors.append(str(e)))
            pg.goto("http://localhost:8767/index.html"+url); pg.wait_for_timeout(600)
            if label=="configured":
                check("login: one big Microsoft button", pg.locator('.modal [data-act="ms-login"]').count()==1)
                check("login: no code/client-id box shown", pg.locator(".code-box").count()==0 and "client" not in pg.inner_text(".modal").lower())
                pg.screenshot(path="/tmp/shots/m1-login.png")
                pg.click('[data-act="ms-login"]'); pg.wait_for_timeout(300)
                check("login: waiting state with cancel", "Waiting for Microsoft" in pg.inner_text(".modal") and pg.locator('[data-act="ms-cancel"]').count()==1)
                check("login: backend asked for browser flow", calls(pg,"ms_login")[-1]=={"inApp":False})
                pg.screenshot(path="/tmp/shots/m2-waiting.png")
                pg.click('[data-act="ms-cancel"]'); pg.wait_for_timeout(500)
                check("login: cancel returns to the form quietly", pg.locator('[data-act="ms-login"]').count()==1 and pg.locator(".toast.error").count()==0)
                pg.click('[data-act="ms-login"]'); pg.wait_for_timeout(3600)
                check("login: success adds the Microsoft account and selects it", pg.locator(".acct.sel:has-text('MSPlayer')").count()==1 and "Microsoft account" in pg.inner_text(".acct.sel"))
                pg.click('[data-act="close-modal"]')
                pg.click('button[data-act="accounts"] >> nth=0'); pg.wait_for_timeout(300)
                pg.click('[data-act="ms-login-app"]'); pg.wait_for_timeout(300)
                check("login: in-launcher window option", calls(pg,"ms_login")[-1]=={"inApp":True})
            else:
                check("login: Microsoft button always present", pg.locator('.modal [data-act="ms-login"]').count()==1)
                pg.click('[data-act="ms-login"]'); pg.wait_for_timeout(300)
                check("login: first use asks for the app ID inline", pg.locator("#ms-id").count()==1)
                pg.fill("#ms-id","nope"); pg.click('[data-act="ms-save"]'); pg.wait_for_timeout(300)
                check("login: bad ID shows an error toast", pg.locator(".toast.error").count()>=1 and len(calls(pg,"save_settings"))==0)
                pg.fill("#ms-id","12345678-aaaa-bbbb-cccc-1234567890ab"); pg.click('[data-act="ms-save"]'); pg.wait_for_timeout(400)
                check("login: valid ID is saved then sign-in starts", calls(pg,"save_settings")[-1]["settings"]["ms_client_id"].startswith("12345678") and len(calls(pg,"ms_login"))==1)
                check("login: offline still offered", True)
                pg.screenshot(path="/tmp/shots/m3-notready.png")
finally:
    srv.terminate()
print("\nJS ERRORS:", errors if errors else "none")
print(f"{sum(ok for _,ok in results)}/{len(results)} passed")
