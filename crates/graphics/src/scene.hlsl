cbuffer Frame : register(b0) {
    float time;
    float width;
    float height;
    float zoom;
    float2 imageSize;
    float channel;
    float motion;
};
struct VOut {
    float4 position : SV_POSITION;
};
VOut vs_main(uint id : SV_VertexID) {
    VOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.position = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}
float4 ps_main(VOut i) : SV_TARGET {
    float2 uv = (i.position.xy * 2 - float2(width, height)) / max(height, 1);
    float3 col =
        lerp(float3(.017, .026, .05), float3(.04, .09, .14), saturate(1 - length(uv) * .35));
    float2 p = uv - float2(.12, -.15);
    float r = length(p);
    col += float3(.08, .55, .70) * .035 / max(abs(r - .69), .018);
    if (r < .69) {
        float z = sqrt(.69 * .69 - r * r);
        float3 n = normalize(float3(p.x, -p.y, z));
        float a = atan2(n.x, n.z) + time * .18;
        float latitude = asin(n.y);
        float lines = pow(abs(sin(a * 13)), 35) + pow(abs(cos(latitude * 15)), 40);
        float lit = max(dot(n, normalize(float3(-.8, .9, 1))), .05);
        col = float3(.025, .10, .14) * lit + float3(.20, .88, .96) * lines * .40 * lit;
        col += pow(1 - n.z, 3) * float3(.1, .7, .85);
        col += pow(max(0, sin(a * 3 + latitude * 7 + time * .4)), 16) * float3(.7, .9, .8) * .1;
    }
    float2 ring = float2(p.x * .85 + p.y * .33, p.y * 3.6 - p.x * .6);
    float orbit = abs(length(ring) - .95);
    if (p.y > .02 || r > .7)
        col += float3(.85, .65, .25) * .004 / max(orbit, .008);
    if (uv.y > .72) {
        float distance = 1 / (uv.y - .62);
        float2 g = float2(uv.x * distance, distance + time * .5);
        float grid = min(abs(frac(g.x) - .5), abs(frac(g.y) - .5));
        col += float3(.07, .3, .36) * smoothstep(.045, 0, grid) * saturate((uv.y - .72) * 2);
    }
    col *= 1 - saturate(dot(uv, uv) * .065);
    return float4(pow(max(col, 0), .85), 1);
}

Texture2D hostTexture : register(t0);
SamplerState linearClamp : register(s0);
float4 ps_texture(VOut input) : SV_TARGET {
    float2 view = float2(width, height);
    float fit = min(width / imageSize.x, height / imageSize.y);
    float scale = fit * zoom * lerp(1.0, 1.12, motion);
    float2 uv = (input.position.xy - view * .5) / (imageSize * scale) + .5;
    uv += motion * float2(.035 * sin(time * .32), .025 * cos(time * .27));
    float checker = fmod(floor(input.position.x / 12) + floor(input.position.y / 12), 2);
    float3 mat = lerp(float3(.075, .082, .09), float3(.089, .096, .104), checker);
    if (any(uv < 0) || any(uv > 1))
        return float4(mat, 1);
    float4 texel = hostTexture.Sample(linearClamp, uv);
    if (channel > .5) {
        float v = channel < 1.5   ? texel.r
                  : channel < 2.5 ? texel.g
                  : channel < 3.5 ? texel.b
                                  : texel.a;
        return float4(v, v, v, 1);
    }
    return float4(lerp(mat, texel.rgb, texel.a), 1);
}
