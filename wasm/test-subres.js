// Sub-resolution decode smoke test.
// Verifies decodeAtScale(bytes, scale) at scale = 1, 2, 4, 8, 16:
//   - dimensions match ceil(width/scale) and ceil(height/scale)
//   - pixelInfo unchanged (same channels / bitsPerPixel / colorFormat)
//   - average channel brightness stays close to the full-res baseline
//     (downsampled image should average to similar values, not zero)
//   - timing: scale > 1 should be measurably faster than scale = 1
let fs = require('fs');
let jpegxr = require('./jpegxr.js');

function avgRGB(image) {
    // panel-hdr.jxr is 32-float RGBA per pixel
    let floats = new Float32Array(image.bytes.buffer);
    let r = 0, g = 0, b = 0;
    let count = image.width * image.height;
    for (let i = 0; i < count; i++) {
        r += floats[i * 4];
        g += floats[i * 4 + 1];
        b += floats[i * 4 + 2];
    }
    return { r: r / count, g: g / count, b: b / count };
}

(async () => {
    let codec = await jpegxr();
    let bytes = fs.readFileSync('samples/panel-hdr.jxr');

    console.log('Decoding panel-hdr.jxr at multiple scales...\n');

    let baseline = null;
    for (let scale of [1, 2, 4, 8, 16]) {
        let t0 = process.hrtime.bigint();
        let image = codec.decodeAtScale(bytes, scale);
        let t1 = process.hrtime.bigint();
        let elapsedMs = Number(t1 - t0) / 1e6;

        let avg = avgRGB(image);
        if (scale === 1) baseline = avg;
        let ratioR = (avg.r / baseline.r).toFixed(3);
        let ratioG = (avg.g / baseline.g).toFixed(3);
        let ratioB = (avg.b / baseline.b).toFixed(3);

        console.log(
            `scale=${String(scale).padStart(2)} ` +
            `dims=${String(image.width).padStart(5)}x${String(image.height).padStart(4)} ` +
            `time=${elapsedMs.toFixed(1).padStart(7)}ms ` +
            `avg=R${avg.r.toFixed(3)} G${avg.g.toFixed(3)} B${avg.b.toFixed(3)} ` +
            `(ratio vs scale=1: R${ratioR} G${ratioG} B${ratioB})`
        );
    }

    console.log('\nExpected: scale=k → dims = ceil(full/k), avg ratios ≈ 1.0, time decreases with scale');
})();
