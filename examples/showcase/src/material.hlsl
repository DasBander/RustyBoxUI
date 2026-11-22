// Approximate two studio lights in reflection space. Roughness widens their highlights.
float3 studio(float3 r, float rough) {
    float blur = .012 + rough * rough * .53;
    float3 light = lerp(float3(.07, .09, .12), float3(.58, .65, .75), saturate(r.y * .65 + .35));
    float strip = 1 - smoothstep(.12 - blur, .12 + blur, abs(r.x + .46));
    strip *= smoothstep(-.15 - blur, -.15 + blur, r.y);
    float broad = smoothstep(.24 - blur, .24 + blur, r.x) * smoothstep(.28 - blur, .28 + blur, r.y);
    float floor = 1 - smoothstep(-.32 - blur, -.32 + blur, r.y);
    return light + float3(2.55, 2.18, 1.85) * strip + float3(1.32, 1.49, 1.68) * broad -
           floor * .053;
}
float4 pixel(float4 pos : SV_POSITION) : SV_TARGET {
    float2 p = (pos.xy - viewport.xy * .5) / viewport.y;
    float rough = max(.018, primary.x), metal = primary.y, orbit = primary.z, tilt = primary.w;
    float3 tint = secondary.rgb;
    float3 backdrop = lerp(float3(.055, .071, .105), float3(.205, .22, .26), saturate(p.y + .58));
    float2 pool = p - float2(-.14, .18);
    backdrop += float3(.085, .081, .074) * exp(-dot(pool, pool) * 5.8);
    float shadow = exp(-pow((p.x - .045) / .26, 2) - pow((p.y - .302) / .043, 2));
    backdrop *= 1 - shadow * .67;
    float2 stand = float2(p.x / .162, (p.y - .286) / .033);
    if (dot(stand, stand) < 1)
        backdrop = lerp(float3(.07, .08, .10), float3(.36, .39, .42),
                        saturate(.60 - stand.y * .33 - stand.x * .24));
    float2 q = (p - float2(0, -.027)) / .297;
    float edge = dot(q, q);
    if (edge > 1)
        return float4(backdrop, 1);
    float3 n = float3(q.x, -q.y, sqrt(saturate(1 - edge)));
    float3 axis =
        normalize(float3(.56 * cos(orbit) - .24, .70 + tilt * .3, .58 * sin(orbit) + .36));
    float lat = dot(n, axis);
    float ring = abs(lat - .34);
    float groove = 1 - smoothstep(.009, .019, ring);
    float3 normal = normalize(n + (axis - lat * n) * (.16 * sign(lat - .34) * groove));
    float3 reflection = reflect(float3(0, 0, -1), normal);
    float angle = orbit * .46;
    reflection.xz = float2(reflection.x * cos(angle) - reflection.z * sin(angle),
                           reflection.x * sin(angle) + reflection.z * cos(angle));
    float lit = .20 + .82 * saturate(dot(normal, normalize(float3(-.55, .83, .64))));
    float3 albedo = pow(max(tint, .001), 2.2);
    float fresnel = pow(1 - saturate(normal.z), 5);
    float3 specular = lerp(.045, albedo, metal);
    specular = lerp(specular, 1.0, fresnel * .75);
    float3 color =
        albedo * lit * (1 - metal * .88) + specular * studio(reflection, rough) * (1 - rough * .35);
    color *= 1 - .58 * groove;
    // Shallow lathed grooves remain visible under grazing light.
    color *= 1 - .018 * metal * sin(lat * 900) * saturate(1 - rough);
    float3 mapped = pow(color / (.78 + color * .48), 1 / 2.2);
    return float4(lerp(backdrop, mapped, saturate((1 - edge) * viewport.y * .21)), 1);
}
