import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import net from 'node:net'
import path from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { chromium } from 'playwright-core'

const root = fileURLToPath(new URL('../', import.meta.url))
const artifacts = path.join(root, 'test-results')
const executable = path.join(root, 'src-tauri/target/debug/packetlens-e1-vertical.exe')

async function unusedLoopbackPort() {
  const server = net.createServer()
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  const port = server.address().port
  await new Promise((resolve) => server.close(resolve))
  return port
}

async function waitForWebView(endpoint, child) {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    assert.equal(child.exitCode, null, 'The native application exited before WebView2 was ready')
    try {
      const response = await fetch(`${endpoint}/json/version`, { signal: AbortSignal.timeout(1000) })
      if (response.ok) return
    } catch {
      // Readiness polling only; application errors and deadline still fail the test.
    }
    await delay(200)
  }
  throw new Error('The native WebView2 CDP endpoint did not become ready within 30 seconds')
}

async function visibleSession(page) {
  return {
    summary: await page.locator('.summary').innerText(),
    source: await page.locator('.trace-banner .source').innerText(),
    rows: await page.locator('.table-wrap tbody').innerText(),
    selection: await page.locator('tr.selected').getAttribute('data-trace-seq'),
    inspector: await page.getByRole('complementary', { name: 'Inspector de evento' }).innerText(),
  }
}

test('native E1-S2 uses real IPC and preserves the displayed live session after a native rejection',
  { timeout: 60_000 }, async () => {
    assert.equal(process.platform, 'win32', 'This acceptance test requires Windows and WebView2')
    await mkdir(artifacts, { recursive: true })
    const profile = await mkdtemp(path.join(root, 'src-tauri/target/webview2-e1-'))
    const endpoint = `http://127.0.0.1:${await unusedLoopbackPort()}`
    const child = spawn(executable, [], {
      cwd: root,
      windowsHide: true,
      stdio: 'inherit',
      env: {
        ...process.env,
        WEBVIEW2_USER_DATA_FOLDER: profile,
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
          `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${new URL(endpoint).port}`,
      },
    })
    let launchError
    child.on('error', (error) => { launchError = error })
    let browser
    let page
    const evidence = { status: 'running', executable, profile, startedAt: new Date().toISOString() }
    try {
      await waitForWebView(endpoint, child)
      if (launchError) throw launchError
      browser = await chromium.connectOverCDP(endpoint)
      const pages = browser.contexts().flatMap((context) => context.pages())
      assert.equal(pages.length, 1, 'The isolated native application must expose exactly one page')
      page = pages[0]
      const pageErrors = []
      page.on('pageerror', (error) => pageErrors.push(error.message))
      await page.getByRole('heading', { name: 'Inspector educativo de traza', exact: true }).waitFor()
      assert.match(page.url(), /^https?:\/\/tauri\.localhost\//,
        'Test the embedded native build, not a Vite server or an external browser')
      assert.match(await page.locator('.trace-banner .source').innerText(), /Snapshot guardado generado por el motor/)

      // Observe the real transport. No fixture or fabricated response is returned.
      await page.evaluate(() => {
        const original = window.fetch
        const probe = { calls: [], rejectNextRun: false, original }
        window.__packetlensAcceptance = probe
        window.fetch = async (input, options) => {
          const url = new URL(input instanceof Request ? input.url : String(input))
          if (url.hostname !== 'ipc.localhost') return original(input, options)
          const command = decodeURIComponent(url.pathname.slice(1))
          const sent = command === 'run_e1_s2_cold' && probe.rejectNextRun
            ? 'packetlens_acceptance_unknown_command' : command
          if (sent !== command) probe.rejectNextRun = false
          url.pathname = `/${sent}`
          const response = await original(url.toString(), options)
          const body = response.headers.get('content-type')?.includes('application/json')
            ? await response.clone().json() : await response.clone().text()
          const status = response.headers.get('Tauri-Response') === 'ok' ? 'fulfilled' : 'rejected'
          probe.calls.push({ command, sent, status,
            ...(status === 'fulfilled' ? { response: body } : { error: String(body) }) })
          return response
        }
      })

      const run = page.getByRole('button', { name: 'Ejecutar motor Rust', exact: true })
      await run.click()
      await page.locator('.trace-banner .source').filter({ hasText: 'Ejecución viva del motor Rust' }).waitFor()
      const calls = await page.evaluate(() => window.__packetlensAcceptance.calls)
      assert.deepEqual(calls.map((call) => [call.command, call.sent, call.status]), [
        ['contract_version', 'contract_version', 'fulfilled'],
        ['run_e1_s2_cold', 'run_e1_s2_cold', 'fulfilled'],
      ])
      assert.equal(calls[0].response, 'presentation/v1')
      const snapshot = JSON.parse(calls[1].response)
      assert.equal(snapshot.schema_version, 'presentation/v1')
      assert.equal(snapshot.scenario_id, 'E1-S2')
      assert.equal(snapshot.run_label, 'cold')
      assert.deepEqual(snapshot.execution, { final_virtual_time_ns: '8700', outcome: 'Quiescent' })
      assert.equal(snapshot.events.length, 27)
      assert.equal(snapshot.trace_retention.total_emitted_ever, '27')
      assert.deepEqual(snapshot.events.map((event) => event.trace_seq),
        Array.from({ length: 27 }, (_, index) => String(index + 1)))
      assert.equal(await page.getByTestId('event-count').innerText(), '27 eventos visibles')
      assert.equal(await page.locator('.controls select').inputValue(), 'live_engine')
      const summary = await page.locator('.summary').innerText()
      assert.match(summary, /Resultado: Quiescent/)
      assert.match(summary, /8\.700 µs/)
      await page.screenshot({ path: path.join(artifacts, 'native-e1-live.png') })

      await page.locator('tr[data-trace-seq="19"]').click()
      await page.getByRole('tab', { name: 'Lectura didáctica', exact: true }).click()
      const beforeError = await visibleSession(page)
      assert.equal(beforeError.selection, '19')
      assert.match(beforeError.inspector, /CAM/)
      await page.evaluate(() => { window.__packetlensAcceptance.rejectNextRun = true })
      await run.click()
      const alert = page.getByRole('alert')
      await alert.waitFor()
      assert.match(await alert.innerText(), /No se reemplazó el snapshot actual/)
      assert.match(await alert.innerText(), /packetlens_acceptance_unknown_command/)
      assert.deepEqual(await visibleSession(page), beforeError,
        'A native IPC rejection must preserve source, summary, trace, selection and inspector')
      const rejected = await page.evaluate(() => window.__packetlensAcceptance.calls.at(-1))
      assert.equal(rejected.sent, 'packetlens_acceptance_unknown_command')
      assert.equal(rejected.status, 'rejected')
      await page.screenshot({ path: path.join(artifacts, 'native-e1-preserved.png') })

      await run.click()
      await alert.waitFor({ state: 'detached' })
      await page.waitForFunction(() => window.__packetlensAcceptance.calls.length === 6)
      const recovered = await page.evaluate(() => window.__packetlensAcceptance.calls.at(-1))
      assert.equal(recovered.command, 'run_e1_s2_cold')
      assert.equal(recovered.sent, 'run_e1_s2_cold')
      assert.equal(recovered.status, 'fulfilled')
      assert.equal(JSON.parse(recovered.response).execution.final_virtual_time_ns, '8700')
      assert.equal(await page.getByTestId('event-count').innerText(), '27 eventos visibles')
      assert.deepEqual(pageErrors, [])
      Object.assign(evidence, {
        status: 'passed', url: page.url(), schemaVersion: snapshot.schema_version,
        eventCount: snapshot.events.length, finalVirtualTimeNs: snapshot.execution.final_virtual_time_ns,
        outcome: snapshot.execution.outcome, source: 'live_engine',
        nativeRejection: rejected.error, sessionPreserved: true, recoveryPassed: true,
        calls: await page.evaluate(() => window.__packetlensAcceptance.calls.map(
          ({ command, sent, status }) => ({ command, sent, status }))),
      })
    } catch (error) {
      Object.assign(evidence, { status: 'failed', error: String(error) })
      if (page) await page.screenshot({ path: path.join(artifacts, 'native-e1-failed.png') }).catch(() => {})
      throw error
    } finally {
      if (page) await page.evaluate(() => {
        window.fetch = window.__packetlensAcceptance.original
        delete window.__packetlensAcceptance
      }).catch(() => {})
      if (browser) await browser.close()
      if (child.pid && child.exitCode === null) {
        execFileSync('taskkill.exe', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore' })
      }
      evidence.finishedAt = new Date().toISOString()
      await writeFile(path.join(artifacts, 'native-e1.json'), `${JSON.stringify(evidence, null, 2)}\n`)
    }
  })
