// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="index.html">The Zuri Standard Library</a></span></li><li class="chapter-item expanded "><li class="part-title">The Language Itself</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="types.html"><strong aria-hidden="true">1.</strong> types</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="enum.html"><strong aria-hidden="true">2.</strong> enum</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="convert.html"><strong aria-hidden="true">3.</strong> convert</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="zuri.html"><strong aria-hidden="true">4.</strong> zuri</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="zuri-ast.html"><strong aria-hidden="true">4.1.</strong> zuri.ast</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="zuri-compile.html"><strong aria-hidden="true">4.2.</strong> zuri.compile</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="zuri-reflect.html"><strong aria-hidden="true">4.3.</strong> zuri.reflect</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="zuri-token.html"><strong aria-hidden="true">4.4.</strong> zuri.token</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Text and Markup</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="html.html"><strong aria-hidden="true">5.</strong> html</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-elements.html"><strong aria-hidden="true">5.1.</strong> html.elements</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-entities.html"><strong aria-hidden="true">5.2.</strong> html.entities</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-namespaces.html"><strong aria-hidden="true">5.3.</strong> html.namespaces</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-node.html"><strong aria-hidden="true">5.4.</strong> html.node</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-parser.html"><strong aria-hidden="true">5.5.</strong> html.parser</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-selector.html"><strong aria-hidden="true">5.6.</strong> html.selector</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-serialize.html"><strong aria-hidden="true">5.7.</strong> html.serialize</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="html-tokenizer.html"><strong aria-hidden="true">5.8.</strong> html.tokenizer</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="wire.html"><strong aria-hidden="true">6.</strong> wire</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-compile.html"><strong aria-hidden="true">6.1.</strong> wire.compile</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-constants.html"><strong aria-hidden="true">6.2.</strong> wire.constants</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-errors.html"><strong aria-hidden="true">6.3.</strong> wire.errors</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-escape.html"><strong aria-hidden="true">6.4.</strong> wire.escape</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-expression.html"><strong aria-hidden="true">6.5.</strong> wire.expression</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-filters.html"><strong aria-hidden="true">6.6.</strong> wire.filters</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-loader.html"><strong aria-hidden="true">6.7.</strong> wire.loader</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-normalize.html"><strong aria-hidden="true">6.8.</strong> wire.normalize</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-render.html"><strong aria-hidden="true">6.9.</strong> wire.render</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="wire-values.html"><strong aria-hidden="true">6.10.</strong> wire.values</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="url.html"><strong aria-hidden="true">7.</strong> url</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="mime.html"><strong aria-hidden="true">8.</strong> mime</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="colors.html"><strong aria-hidden="true">9.</strong> colors</a></span></li><li class="chapter-item expanded "><li class="part-title">Data Formats</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="json.html"><strong aria-hidden="true">10.</strong> json</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="yaml.html"><strong aria-hidden="true">11.</strong> yaml</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="csv.html"><strong aria-hidden="true">12.</strong> csv</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="base64.html"><strong aria-hidden="true">13.</strong> base64</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="struct.html"><strong aria-hidden="true">14.</strong> struct</a></span></li><li class="chapter-item expanded "><li class="part-title">Numbers and Collections</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="math.html"><strong aria-hidden="true">15.</strong> math</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="stat.html"><strong aria-hidden="true">16.</strong> stat</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="array.html"><strong aria-hidden="true">17.</strong> array</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-double.html"><strong aria-hidden="true">17.1.</strong> array.double</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-float.html"><strong aria-hidden="true">17.2.</strong> array.float</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-int16.html"><strong aria-hidden="true">17.3.</strong> array.int16</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-int32.html"><strong aria-hidden="true">17.4.</strong> array.int32</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-int64.html"><strong aria-hidden="true">17.5.</strong> array.int64</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-int8.html"><strong aria-hidden="true">17.6.</strong> array.int8</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-uint16.html"><strong aria-hidden="true">17.7.</strong> array.uint16</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-uint32.html"><strong aria-hidden="true">17.8.</strong> array.uint32</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-uint64.html"><strong aria-hidden="true">17.9.</strong> array.uint64</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="array-uint8.html"><strong aria-hidden="true">17.10.</strong> array.uint8</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="set.html"><strong aria-hidden="true">18.</strong> set</a></span></li><li class="chapter-item expanded "><li class="part-title">Dates and Times</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="date.html"><strong aria-hidden="true">19.</strong> date</a></span></li><li class="chapter-item expanded "><li class="part-title">The Machine</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="os.html"><strong aria-hidden="true">20.</strong> os</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-env.html"><strong aria-hidden="true">20.1.</strong> os.env</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-fs.html"><strong aria-hidden="true">20.2.</strong> os.fs</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-path.html"><strong aria-hidden="true">20.3.</strong> os.path</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-process.html"><strong aria-hidden="true">20.4.</strong> os.process</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-system.html"><strong aria-hidden="true">20.5.</strong> os.system</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="os-tempfile.html"><strong aria-hidden="true">20.6.</strong> os.tempfile</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="io.html"><strong aria-hidden="true">21.</strong> io</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="io-bytesio.html"><strong aria-hidden="true">21.1.</strong> io.bytesio</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="io-tty.html"><strong aria-hidden="true">21.2.</strong> io.tty</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="args.html"><strong aria-hidden="true">22.</strong> args</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="log.html"><strong aria-hidden="true">23.</strong> log</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-console.html"><strong aria-hidden="true">23.1.</strong> log.console</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-dispatch.html"><strong aria-hidden="true">23.2.</strong> log.dispatch</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-file.html"><strong aria-hidden="true">23.3.</strong> log.file</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-level.html"><strong aria-hidden="true">23.4.</strong> log.level</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-logger.html"><strong aria-hidden="true">23.5.</strong> log.logger</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="log-transport.html"><strong aria-hidden="true">23.6.</strong> log.transport</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Networking</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="net.html"><strong aria-hidden="true">24.</strong> net</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-addr.html"><strong aria-hidden="true">24.1.</strong> net.addr</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-dtls.html"><strong aria-hidden="true">24.2.</strong> net.dtls</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-ip.html"><strong aria-hidden="true">24.3.</strong> net.ip</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-poll.html"><strong aria-hidden="true">24.4.</strong> net.poll</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-tcp.html"><strong aria-hidden="true">24.5.</strong> net.tcp</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-tls.html"><strong aria-hidden="true">24.6.</strong> net.tls</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="net-udp.html"><strong aria-hidden="true">24.7.</strong> net.udp</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="http.html"><strong aria-hidden="true">25.</strong> http</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-body.html"><strong aria-hidden="true">25.1.</strong> http.body</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-client.html"><strong aria-hidden="true">25.2.</strong> http.client</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-cookies.html"><strong aria-hidden="true">25.3.</strong> http.cookies</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-errors.html"><strong aria-hidden="true">25.4.</strong> http.errors</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-files.html"><strong aria-hidden="true">25.5.</strong> http.files</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h1.html"><strong aria-hidden="true">25.6.</strong> http.h1</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h2.html"><strong aria-hidden="true">25.7.</strong> http.h2</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h2-connection.html"><strong aria-hidden="true">25.7.1.</strong> http.h2.connection</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h2-frames.html"><strong aria-hidden="true">25.7.2.</strong> http.h2.frames</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h2-hpack.html"><strong aria-hidden="true">25.7.3.</strong> http.h2.hpack</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-h2-huffman.html"><strong aria-hidden="true">25.7.4.</strong> http.h2.huffman</a></span></li></ol><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-headers.html"><strong aria-hidden="true">25.8.</strong> http.headers</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-middleware.html"><strong aria-hidden="true">25.9.</strong> http.middleware</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-multipart.html"><strong aria-hidden="true">25.10.</strong> http.multipart</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-negotiate.html"><strong aria-hidden="true">25.11.</strong> http.negotiate</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-proxy.html"><strong aria-hidden="true">25.12.</strong> http.proxy</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-request.html"><strong aria-hidden="true">25.13.</strong> http.request</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-response.html"><strong aria-hidden="true">25.14.</strong> http.response</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-router.html"><strong aria-hidden="true">25.15.</strong> http.router</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-server.html"><strong aria-hidden="true">25.16.</strong> http.server</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-sse.html"><strong aria-hidden="true">25.17.</strong> http.sse</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-status.html"><strong aria-hidden="true">25.18.</strong> http.status</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-stream.html"><strong aria-hidden="true">25.19.</strong> http.stream</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-util.html"><strong aria-hidden="true">25.20.</strong> http.util</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-websocket.html"><strong aria-hidden="true">25.21.</strong> http.websocket</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="http-worker.html"><strong aria-hidden="true">25.22.</strong> http.worker</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Security and Identity</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="crypto.html"><strong aria-hidden="true">26.</strong> crypto</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="hash.html"><strong aria-hidden="true">27.</strong> hash</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="bcrypt.html"><strong aria-hidden="true">28.</strong> bcrypt</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="jwt.html"><strong aria-hidden="true">29.</strong> jwt</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-codec.html"><strong aria-hidden="true">29.1.</strong> jwt.codec</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-core.html"><strong aria-hidden="true">29.2.</strong> jwt.core</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-errors.html"><strong aria-hidden="true">29.3.</strong> jwt.errors</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-jwks.html"><strong aria-hidden="true">29.4.</strong> jwt.jwks</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-signer.html"><strong aria-hidden="true">29.5.</strong> jwt.signer</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-token.html"><strong aria-hidden="true">29.6.</strong> jwt.token</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="jwt-verifier.html"><strong aria-hidden="true">29.7.</strong> jwt.verifier</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="uuid.html"><strong aria-hidden="true">30.</strong> uuid</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="validate.html"><strong aria-hidden="true">31.</strong> validate</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="validate-rule.html"><strong aria-hidden="true">31.1.</strong> validate.rule</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="validate-rules.html"><strong aria-hidden="true">31.2.</strong> validate.rules</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="validate-schema.html"><strong aria-hidden="true">31.3.</strong> validate.schema</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="validate-validator.html"><strong aria-hidden="true">31.4.</strong> validate.validator</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="validate-validators.html"><strong aria-hidden="true">31.5.</strong> validate.validators</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Compression and Archives</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="compress.html"><strong aria-hidden="true">32.</strong> compress</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-brotli.html"><strong aria-hidden="true">32.1.</strong> compress.brotli</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-bzip2.html"><strong aria-hidden="true">32.2.</strong> compress.bzip2</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-checksum.html"><strong aria-hidden="true">32.3.</strong> compress.checksum</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-deflate.html"><strong aria-hidden="true">32.4.</strong> compress.deflate</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-gzip.html"><strong aria-hidden="true">32.5.</strong> compress.gzip</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-lz4.html"><strong aria-hidden="true">32.6.</strong> compress.lz4</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-tar.html"><strong aria-hidden="true">32.7.</strong> compress.tar</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-zip.html"><strong aria-hidden="true">32.8.</strong> compress.zip</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-zlib.html"><strong aria-hidden="true">32.9.</strong> compress.zlib</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="compress-zstd.html"><strong aria-hidden="true">32.10.</strong> compress.zstd</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Concurrency</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="isolate.html"><strong aria-hidden="true">33.</strong> isolate</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="isolate-broadcast.html"><strong aria-hidden="true">33.1.</strong> isolate.broadcast</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="isolate-channel.html"><strong aria-hidden="true">33.2.</strong> isolate.channel</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="isolate-error.html"><strong aria-hidden="true">33.3.</strong> isolate.error</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Images</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="imagine.html"><strong aria-hidden="true">34.</strong> imagine</a><a class="chapter-fold-toggle"><div>❱</div></a></span><ol class="section"><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-animation.html"><strong aria-hidden="true">34.1.</strong> imagine.animation</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-canvas.html"><strong aria-hidden="true">34.2.</strong> imagine.canvas</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-color.html"><strong aria-hidden="true">34.3.</strong> imagine.color</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-constants.html"><strong aria-hidden="true">34.4.</strong> imagine.constants</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-errors.html"><strong aria-hidden="true">34.5.</strong> imagine.errors</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-filters.html"><strong aria-hidden="true">34.6.</strong> imagine.filters</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-font.html"><strong aria-hidden="true">34.7.</strong> imagine.font</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-formats.html"><strong aria-hidden="true">34.8.</strong> imagine.formats</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-image.html"><strong aria-hidden="true">34.9.</strong> imagine.image</a></span></li><li class="chapter-item "><span class="chapter-link-wrapper"><a href="imagine-strokefont.html"><strong aria-hidden="true">34.10.</strong> imagine.strokefont</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);


// ---------------------------------------------------------------------------
// Support for dynamically adding headers to the sidebar.

(function() {
    // This is used to detect which direction the page has scrolled since the
    // last scroll event.
    let lastKnownScrollPosition = 0;
    // This is the threshold in px from the top of the screen where it will
    // consider a header the "current" header when scrolling down.
    const defaultDownThreshold = 150;
    // Same as defaultDownThreshold, except when scrolling up.
    const defaultUpThreshold = 300;
    // The threshold is a virtual horizontal line on the screen where it
    // considers the "current" header to be above the line. The threshold is
    // modified dynamically to handle headers that are near the bottom of the
    // screen, and to slightly offset the behavior when scrolling up vs down.
    let threshold = defaultDownThreshold;
    // This is used to disable updates while scrolling. This is needed when
    // clicking the header in the sidebar, which triggers a scroll event. It
    // is somewhat finicky to detect when the scroll has finished, so this
    // uses a relatively dumb system of disabling scroll updates for a short
    // time after the click.
    let disableScroll = false;
    // Array of header elements on the page.
    let headers;
    // Array of li elements that are initially collapsed headers in the sidebar.
    // I'm not sure why eslint seems to have a false positive here.
    // eslint-disable-next-line prefer-const
    let headerToggles = [];
    // This is a debugging tool for the threshold which you can enable in the console.
    let thresholdDebug = false;

    // Updates the threshold based on the scroll position.
    function updateThreshold() {
        const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
        const windowHeight = window.innerHeight;
        const documentHeight = document.documentElement.scrollHeight;

        // The number of pixels below the viewport, at most documentHeight.
        // This is used to push the threshold down to the bottom of the page
        // as the user scrolls towards the bottom.
        const pixelsBelow = Math.max(0, documentHeight - (scrollTop + windowHeight));
        // The number of pixels above the viewport, at least defaultDownThreshold.
        // Similar to pixelsBelow, this is used to push the threshold back towards
        // the top when reaching the top of the page.
        const pixelsAbove = Math.max(0, defaultDownThreshold - scrollTop);
        // How much the threshold should be offset once it gets close to the
        // bottom of the page.
        const bottomAdd = Math.max(0, windowHeight - pixelsBelow - defaultDownThreshold);
        let adjustedBottomAdd = bottomAdd;

        // Adjusts bottomAdd for a small document. The calculation above
        // assumes the document is at least twice the windowheight in size. If
        // it is less than that, then bottomAdd needs to be shrunk
        // proportional to the difference in size.
        if (documentHeight < windowHeight * 2) {
            const maxPixelsBelow = documentHeight - windowHeight;
            const t = 1 - pixelsBelow / Math.max(1, maxPixelsBelow);
            const clamp = Math.max(0, Math.min(1, t));
            adjustedBottomAdd *= clamp;
        }

        let scrollingDown = true;
        if (scrollTop < lastKnownScrollPosition) {
            scrollingDown = false;
        }

        if (scrollingDown) {
            // When scrolling down, move the threshold up towards the default
            // downwards threshold position. If near the bottom of the page,
            // adjustedBottomAdd will offset the threshold towards the bottom
            // of the page.
            const amountScrolledDown = scrollTop - lastKnownScrollPosition;
            const adjustedDefault = defaultDownThreshold + adjustedBottomAdd;
            threshold = Math.max(adjustedDefault, threshold - amountScrolledDown);
        } else {
            // When scrolling up, move the threshold down towards the default
            // upwards threshold position. If near the bottom of the page,
            // quickly transition the threshold back up where it normally
            // belongs.
            const amountScrolledUp = lastKnownScrollPosition - scrollTop;
            const adjustedDefault = defaultUpThreshold - pixelsAbove
                + Math.max(0, adjustedBottomAdd - defaultDownThreshold);
            threshold = Math.min(adjustedDefault, threshold + amountScrolledUp);
        }

        if (documentHeight <= windowHeight) {
            threshold = 0;
        }

        if (thresholdDebug) {
            const id = 'mdbook-threshold-debug-data';
            let data = document.getElementById(id);
            if (data === null) {
                data = document.createElement('div');
                data.id = id;
                data.style.cssText = `
                    position: fixed;
                    top: 50px;
                    right: 10px;
                    background-color: 0xeeeeee;
                    z-index: 9999;
                    pointer-events: none;
                `;
                document.body.appendChild(data);
            }
            data.innerHTML = `
                <table>
                  <tr><td>documentHeight</td><td>${documentHeight.toFixed(1)}</td></tr>
                  <tr><td>windowHeight</td><td>${windowHeight.toFixed(1)}</td></tr>
                  <tr><td>scrollTop</td><td>${scrollTop.toFixed(1)}</td></tr>
                  <tr><td>pixelsAbove</td><td>${pixelsAbove.toFixed(1)}</td></tr>
                  <tr><td>pixelsBelow</td><td>${pixelsBelow.toFixed(1)}</td></tr>
                  <tr><td>bottomAdd</td><td>${bottomAdd.toFixed(1)}</td></tr>
                  <tr><td>adjustedBottomAdd</td><td>${adjustedBottomAdd.toFixed(1)}</td></tr>
                  <tr><td>scrollingDown</td><td>${scrollingDown}</td></tr>
                  <tr><td>threshold</td><td>${threshold.toFixed(1)}</td></tr>
                </table>
            `;
            drawDebugLine();
        }

        lastKnownScrollPosition = scrollTop;
    }

    function drawDebugLine() {
        if (!document.body) {
            return;
        }
        const id = 'mdbook-threshold-debug-line';
        const existingLine = document.getElementById(id);
        if (existingLine) {
            existingLine.remove();
        }
        const line = document.createElement('div');
        line.id = id;
        line.style.cssText = `
            position: fixed;
            top: ${threshold}px;
            left: 0;
            width: 100vw;
            height: 2px;
            background-color: red;
            z-index: 9999;
            pointer-events: none;
        `;
        document.body.appendChild(line);
    }

    function mdbookEnableThresholdDebug() {
        thresholdDebug = true;
        updateThreshold();
        drawDebugLine();
    }

    window.mdbookEnableThresholdDebug = mdbookEnableThresholdDebug;

    // Updates which headers in the sidebar should be expanded. If the current
    // header is inside a collapsed group, then it, and all its parents should
    // be expanded.
    function updateHeaderExpanded(currentA) {
        // Add expanded to all header-item li ancestors.
        let current = currentA.parentElement;
        while (current) {
            if (current.tagName === 'LI' && current.classList.contains('header-item')) {
                current.classList.add('expanded');
            }
            current = current.parentElement;
        }
    }

    // Updates which header is marked as the "current" header in the sidebar.
    // This is done with a virtual Y threshold, where headers at or below
    // that line will be considered the current one.
    function updateCurrentHeader() {
        if (!headers || !headers.length) {
            return;
        }

        // Reset the classes, which will be rebuilt below.
        const els = document.getElementsByClassName('current-header');
        for (const el of els) {
            el.classList.remove('current-header');
        }
        for (const toggle of headerToggles) {
            toggle.classList.remove('expanded');
        }

        // Find the last header that is above the threshold.
        let lastHeader = null;
        for (const header of headers) {
            const rect = header.getBoundingClientRect();
            if (rect.top <= threshold) {
                lastHeader = header;
            } else {
                break;
            }
        }
        if (lastHeader === null) {
            lastHeader = headers[0];
            const rect = lastHeader.getBoundingClientRect();
            const windowHeight = window.innerHeight;
            if (rect.top >= windowHeight) {
                return;
            }
        }

        // Get the anchor in the summary.
        const href = '#' + lastHeader.id;
        const a = [...document.querySelectorAll('.header-in-summary')]
            .find(element => element.getAttribute('href') === href);
        if (!a) {
            return;
        }

        a.classList.add('current-header');

        updateHeaderExpanded(a);
    }

    // Updates which header is "current" based on the threshold line.
    function reloadCurrentHeader() {
        if (disableScroll) {
            return;
        }
        updateThreshold();
        updateCurrentHeader();
    }


    // When clicking on a header in the sidebar, this adjusts the threshold so
    // that it is located next to the header. This is so that header becomes
    // "current".
    function headerThresholdClick(event) {
        // See disableScroll description why this is done.
        disableScroll = true;
        setTimeout(() => {
            disableScroll = false;
        }, 100);
        // requestAnimationFrame is used to delay the update of the "current"
        // header until after the scroll is done, and the header is in the new
        // position.
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                // Closest is needed because if it has child elements like <code>.
                const a = event.target.closest('a');
                const href = a.getAttribute('href');
                const targetId = href.substring(1);
                const targetElement = document.getElementById(targetId);
                if (targetElement) {
                    threshold = targetElement.getBoundingClientRect().bottom;
                    updateCurrentHeader();
                }
            });
        });
    }

    // Takes the nodes from the given head and copies them over to the
    // destination, along with some filtering.
    function filterHeader(source, dest) {
        const clone = source.cloneNode(true);
        clone.querySelectorAll('mark').forEach(mark => {
            mark.replaceWith(...mark.childNodes);
        });
        dest.append(...clone.childNodes);
    }

    // Scans page for headers and adds them to the sidebar.
    document.addEventListener('DOMContentLoaded', function() {
        const activeSection = document.querySelector('#mdbook-sidebar .active');
        if (activeSection === null) {
            return;
        }

        const main = document.getElementsByTagName('main')[0];
        headers = Array.from(main.querySelectorAll('h2, h3, h4, h5, h6'))
            .filter(h => h.id !== '' && h.children.length && h.children[0].tagName === 'A');

        if (headers.length === 0) {
            return;
        }

        // Build a tree of headers in the sidebar.

        const stack = [];

        const firstLevel = parseInt(headers[0].tagName.charAt(1));
        for (let i = 1; i < firstLevel; i++) {
            const ol = document.createElement('ol');
            ol.classList.add('section');
            if (stack.length > 0) {
                stack[stack.length - 1].ol.appendChild(ol);
            }
            stack.push({level: i + 1, ol: ol});
        }

        // The level where it will start folding deeply nested headers.
        const foldLevel = 3;

        for (let i = 0; i < headers.length; i++) {
            const header = headers[i];
            const level = parseInt(header.tagName.charAt(1));

            const currentLevel = stack[stack.length - 1].level;
            if (level > currentLevel) {
                // Begin nesting to this level.
                for (let nextLevel = currentLevel + 1; nextLevel <= level; nextLevel++) {
                    const ol = document.createElement('ol');
                    ol.classList.add('section');
                    const last = stack[stack.length - 1];
                    const lastChild = last.ol.lastChild;
                    // Handle the case where jumping more than one nesting
                    // level, which doesn't have a list item to place this new
                    // list inside of.
                    if (lastChild) {
                        lastChild.appendChild(ol);
                    } else {
                        last.ol.appendChild(ol);
                    }
                    stack.push({level: nextLevel, ol: ol});
                }
            } else if (level < currentLevel) {
                while (stack.length > 1 && stack[stack.length - 1].level > level) {
                    stack.pop();
                }
            }

            const li = document.createElement('li');
            li.classList.add('header-item');
            li.classList.add('expanded');
            if (level < foldLevel) {
                li.classList.add('expanded');
            }
            const span = document.createElement('span');
            span.classList.add('chapter-link-wrapper');
            const a = document.createElement('a');
            span.appendChild(a);
            a.href = '#' + header.id;
            a.classList.add('header-in-summary');
            filterHeader(header.children[0], a);
            a.addEventListener('click', headerThresholdClick);
            const nextHeader = headers[i + 1];
            if (nextHeader !== undefined) {
                const nextLevel = parseInt(nextHeader.tagName.charAt(1));
                if (nextLevel > level && level >= foldLevel) {
                    const toggle = document.createElement('a');
                    toggle.classList.add('chapter-fold-toggle');
                    toggle.classList.add('header-toggle');
                    toggle.addEventListener('click', () => {
                        li.classList.toggle('expanded');
                    });
                    const toggleDiv = document.createElement('div');
                    toggleDiv.textContent = '❱';
                    toggle.appendChild(toggleDiv);
                    span.appendChild(toggle);
                    headerToggles.push(li);
                }
            }
            li.appendChild(span);

            const currentParent = stack[stack.length - 1];
            currentParent.ol.appendChild(li);
        }

        const onThisPage = document.createElement('div');
        onThisPage.classList.add('on-this-page');
        onThisPage.append(stack[0].ol);
        const activeItemSpan = activeSection.parentElement;
        activeItemSpan.after(onThisPage);
    });

    document.addEventListener('DOMContentLoaded', reloadCurrentHeader);
    document.addEventListener('scroll', reloadCurrentHeader, { passive: true });
})();

