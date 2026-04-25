import json, struct

p = 'assets/models/rolex_datejust.glb'
b = open(p, 'rb').read()
off = 12
jl, ct = struct.unpack_from('<II', b, off)
js = json.loads(b[off+8:off+8+jl].decode('utf-8'))

print('=== MESH PRIMITIVES -> MATERIAL ===')
for mi, mesh in enumerate(js['meshes']):
    mname = mesh.get('name', '?')
    for pi, prim in enumerate(mesh['primitives']):
        mat = prim.get('material', '?')
        matname = js['materials'][mat]['name'] if isinstance(mat, int) else '?'
        print(f'Mesh{mi} ({mname}) prim{pi} -> mat{mat} ({matname})')

print()
print('=== NODES ===')
for ni, node in enumerate(js['nodes']):
    nname = node.get('name', '?')
    mesh = node.get('mesh')
    mname = js['meshes'][mesh]['name'] if mesh is not None else None
    print(f'N{ni} ({nname}) mesh={mname}')

print()
print('=== KHR_materials_specular detail for dial (M2) ===')
m2 = js['materials'][2]
ext = m2.get('extensions', {})
print('name:', m2['name'])
print('extensions:', ext)
print('pbrMetallicRoughness:', m2.get('pbrMetallicRoughness'))
print('normalTexture:', m2.get('normalTexture'))
print('occlusionTexture:', m2.get('occlusionTexture'))
print('emissiveFactor:', m2.get('emissiveFactor'))
