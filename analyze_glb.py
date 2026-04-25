import json, struct, io
from PIL import Image
from collections import Counter

p = 'assets/models/rolex_datejust.glb'
b = open(p, 'rb').read()
off = 12
jl, ct = struct.unpack_from('<II', b, off)
js = json.loads(b[off+8:off+8+jl].decode('utf-8'))
off = off + 8 + ((jl + 3) // 4) * 4
bl, bt = struct.unpack_from('<II', b, off)
bin0 = b[off+8:off+8+bl]

print('=== MATERIALS ===')
for i, m in enumerate(js['materials']):
    pbr = m.get('pbrMetallicRoughness', {})
    bct = pbr.get('baseColorTexture', {}).get('index') if 'baseColorTexture' in pbr else None
    bcf = pbr.get('baseColorFactor', [1,1,1,1])
    met = pbr.get('metallicFactor', 1.0)
    rou = pbr.get('roughnessFactor', 1.0)
    alpha = m.get('alphaMode', 'OPAQUE')
    cut = m.get('alphaCutoff', 0.5)
    ds = m.get('doubleSided', False)
    ext = list(m.get('extensions', {}).keys())
    name = m['name']
    print(f'M{i} {name}: bcf={[round(x,3) for x in bcf]} bct={bct} met={met:.2f} rou={rou:.2f} alpha={alpha} cut={cut} ds={ds} ext={ext}')

print()
print('=== TEXTURES ===')
for i, t in enumerate(js['textures']):
    print(f'T{i} -> img={t["source"]} sampler={t.get("sampler")}')

print()
print('=== IMAGE COLORS ===')
for img_idx in range(len(js['images'])):
    bv = js['bufferViews'][js['images'][img_idx]['bufferView']]
    data = bin0[bv.get('byteOffset', 0):bv.get('byteOffset', 0)+bv['byteLength']]
    im = Image.open(io.BytesIO(data)).convert('RGBA')
    px = list(im.getdata())
    opaque = [tuple(p[:3]) for p in px if p[3] > 200]
    top = Counter(opaque).most_common(3) if opaque else []
    print(f'IMG{img_idx} {im.size} opaque={len(opaque)}/{len(px)} top={top}')

print()
print('=== SAMPLERS ===')
for i, s in enumerate(js.get('samplers', [])):
    print(f'Sampler{i}: {s}')
