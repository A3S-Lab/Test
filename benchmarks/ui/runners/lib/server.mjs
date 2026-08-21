import { createReadStream, promises as fs } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

const CONTENT_TYPES = {
  ".css": "text/css; charset=utf-8",
  ".gif": "image/gif",
  ".html": "text/html; charset=utf-8",
  ".jpeg": "image/jpeg",
  ".jpg": "image/jpeg",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

export async function startMiniwobServer(miniwobRepositoryRoot) {
  const htmlRoot = resolve(miniwobRepositoryRoot, "miniwob/html");
  await verifyMiniwobRoot(htmlRoot);

  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const pathname = decodeURIComponent(url.pathname);
      const filePath = resolve(htmlRoot, `.${pathname}`);
      if (filePath !== htmlRoot && !filePath.startsWith(`${htmlRoot}${sep}`)) {
        respond(response, 403, "Path escapes the MiniWoB root.");
        return;
      }

      const stat = await fs.stat(filePath);
      if (!stat.isFile()) {
        respond(response, 404, "Not found.");
        return;
      }

      response.setHeader("Cache-Control", "no-store");
      response.setHeader(
        "Content-Type",
        CONTENT_TYPES[extname(filePath).toLowerCase()] ??
          "application/octet-stream",
      );

      if (isMiniwobTask(pathname)) {
        const html = await fs.readFile(filePath, "utf8");
        response.end(injectBenchmarkBootstrap(html));
        return;
      }

      createReadStream(filePath).pipe(response);
    } catch (error) {
      if (error?.code === "ENOENT") {
        respond(response, 404, "Not found.");
        return;
      }
      respond(response, 500, `MiniWoB server error: ${error.message}`);
    }
  });

  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("MiniWoB server did not expose a TCP address.");
  }

  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    close: () => new Promise((resolveClose) => server.close(resolveClose)),
  };
}

function isMiniwobTask(pathname) {
  return /^\/miniwob\/[^/]+\.html$/.test(pathname);
}

function injectBenchmarkBootstrap(html) {
  const marker = "</body>";
  if (!html.includes(marker)) {
    throw new Error("MiniWoB task has no closing body element.");
  }

  return html.replace(marker, `${BOOTSTRAP_SCRIPT}\n${marker}`);
}

const BOOTSTRAP_SCRIPT = String.raw`<script id="a3s-ui-benchmark-bootstrap">
(() => {
  const taskOnLoad = window.onload;
  window.onload = function benchmarkOnLoad(event) {
    if (typeof taskOnLoad === 'function') {
      taskOnLoad.call(window, event);
    }

    const params = new URLSearchParams(window.location.search);
    const seed = Number(params.get('seed'));
    const episodeMaxTime = Number(params.get('episode_max_time') || '120000');
    if (!Number.isSafeInteger(seed) || seed < 0) {
      throw new Error('The UI benchmark requires a non-negative integer seed.');
    }
    if (!Number.isSafeInteger(episodeMaxTime) || episodeMaxTime <= 0) {
      throw new Error('The UI benchmark requires a positive episode timeout.');
    }

    const result = document.createElement('span');
    result.id = '__a3s_ui_benchmark_result';
    result.setAttribute('aria-hidden', 'true');
    result.setAttribute('data-benchmark-done', 'false');
    result.setAttribute('data-benchmark-pass', 'false');
    result.setAttribute('data-benchmark-raw-reward', '0');
    result.style.cssText = [
      'position:fixed',
      'display:block',
      'right:0',
      'bottom:0',
      'width:1px',
      'height:1px',
      'overflow:hidden',
      'color:transparent',
      'background:rgba(0,0,0,0.001)',
      'pointer-events:none',
      'z-index:2147483647',
    ].join(';');
    document.body.appendChild(result);

    const originalEndEpisode = core.endEpisode;
    core.endEpisode = function benchmarkEndEpisode(reward, timeProportional, reason) {
      result.setAttribute('data-benchmark-done', 'true');
      result.setAttribute('data-benchmark-pass', reward > 0 ? 'true' : 'false');
      result.setAttribute('data-benchmark-raw-reward', String(reward));
      return originalEndEpisode.call(core, reward, timeProportional, reason);
    };

    Math.seedrandom(seed);
    core.EPISODE_MAX_TIME = episodeMaxTime;
    core.startEpisodeReal();

    for (const id of ['reward-display', 'click-canvas', 'sync-task-cover']) {
      const element = document.getElementById(id);
      if (element) {
        element.setAttribute('aria-hidden', 'true');
        element.style.display = 'none';
      }
    }
  };
})();
</script>`;

async function verifyMiniwobRoot(htmlRoot) {
  const required = [
    resolve(htmlRoot, "core/core.js"),
    resolve(htmlRoot, "miniwob/click-button.html"),
  ];
  for (const filePath of required) {
    const stat = await fs.stat(filePath);
    if (!stat.isFile()) {
      throw new Error(`Missing MiniWoB fixture: ${filePath}`);
    }
  }
}

function respond(response, statusCode, body) {
  response.statusCode = statusCode;
  response.setHeader("Content-Type", "text/plain; charset=utf-8");
  response.end(body);
}
