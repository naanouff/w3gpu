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

# Check all images that have transparent pixels  
images_to_check = [6, 7, 8, 9]

for img_idx in images_to_check:
    bv = js['bufferViews'][js['images'][img_idx]['bufferView']]
    data = bin0[bv.get('byteOffset', 0):bv.get('byteOffset', 0)+bv['byteLength']]
    im = Image.open(io.BytesIO(data)).convert('RGBA')
    pixels = list(im.getdata())
    
    transparent = [tuple(p[:3]) for p in pixels if p[3] < 8]
    opaque = [tuple(p[:3]) for p in pixels if p[3] >= 128]
    semi = [tuple(p[:3]) for p in pixels if 8 <= p[3] < 128]
    
    print(f"\nIMG{img_idx} {im.size}:")
    print(f"  transparent(alpha<8): {len(transparent)}, opaque(>=128): {len(opaque)}, semi: {len(semi)}")
    
    if transparent:
        top_trans = Counter(transparent).most_common(5)
        print(f"  top transparent RGB: {top_trans}")
        # Sample a few raw transparent pixels to see their actual alpha too
        raw_trans = [(p[:3], p[3]) for p in pixels if p[3] < 8][:10]
        print(f"  first 10 transparent (rgb, alpha): {raw_trans}")
    
    if opaque:
        top_opaque = Counter(opaque).most_common(5)
        print(f"  top opaque RGB: {top_opaque}")
