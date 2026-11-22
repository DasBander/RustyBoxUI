// Perspective road on a repeating 1.8 km desert circuit.
float ridge(float x, float seed) {
    float sum = 0, amplitude = .5;
    [unroll] for (int i = 0; i < 5; i++) {
        float cell = floor(x);
        float a = frac(sin(cell * 53.41 + seed) * 4375.39);
        float b = frac(sin((cell + 1) * 53.41 + seed) * 4375.39);
        sum += lerp(a, b, frac(x)) * amplitude;
        amplitude *= .50;
        x = x * 2.21 + 3.17;
    }
    return sum;
}
float box(float2 p, float2 radius) {
    float2 d = abs(p) - radius;
    return length(max(d, 0)) + min(max(d.x, d.y), 0);
}
float rounded(float2 p, float2 halfSize, float radius) {
    return box(p, halfSize - radius) - radius;
}
float fillShape(float d) {
    return 1 - smoothstep(-.0007, .0007, d);
}
float4 pixel(float4 pos : SV_POSITION) : SV_TARGET {
    float2 uv = pos.xy / viewport.xy;
    float aspect = viewport.x / viewport.y;
    float x = (uv.x - .5) * aspect;
    float distance = primary.x, lane = primary.y, curve = primary.z, heading = primary.w;
    float horizon = .425;
    float altitude = saturate(uv.y / horizon);
    float3 sky = lerp(float3(.075, .20, .59), float3(.31, .62, .91), altitude);
    sky = lerp(sky, float3(1, .66, .41), smoothstep(.58, 1., altitude));
    float2 sun = float2(x + .37 + .05 * sin(heading), (uv.y - .235));
    sky += float3(.15, .09, .025) * exp(-length(sun) * 10);
    sky = lerp(sky, float3(1, .96, .73), 1 - smoothstep(.042, .044, length(sun)));
    float m = x + heading * .19;
    float farPeak = .43 - .18 * ridge(m * 4.0 + 15, 2.5);
    float nearPeak = .46 - .22 * ridge(m * 4.8 + 23, 33.7);
    float3 col = sky;
    if (uv.y > farPeak)
        col = lerp(float3(.32, .27, .48), float3(.72, .41, .49), saturate((uv.y - farPeak) * 9));
    if (uv.y > nearPeak) {
        float relief = ridge(m * 35 + uv.y * 12, 1.3);
        col = lerp(float3(.37, .13, .18), float3(.94, .43, .20),
                   saturate(relief * 1.45 + (uv.y - nearPeak) * 2.5));
    }
    if (uv.y > horizon) {
        float depth = (uv.y - horizon) / (1 - horizon);
        float z = 6.4 / max(depth, .0001);
        float bend = curve * .25 * pow(1 - depth, 2);
        float roadCenter = bend - lane * depth * .46;
        float across = (x - roadCenter) / max(depth * .91, .0001);
        float a = abs(across);
        float world = z + distance;
        float grain = frac(sin(dot(floor(pos.xy), float2(15.73, 51.29))) * 4317.327) - .5;
        float3 sand = lerp(float3(.96, .59, .30), float3(.75, .33, .14),
                           saturate(depth + .10 * sin(world * .36)));
        sand += .036 * ridge(across * 4 + world * .19, 10.7) + grain * .020;
        float3 asphalt =
            lerp(float3(.37, .37, .43), float3(.095, .11, .16), pow(depth, .43)) + grain * .024;
        asphalt *= 1 - .1 * exp(-pow((a - .48) * 5, 2));
        float stripe =
            smoothstep(.005, .014, abs(frac(world / 8) - .5)) * step(.50, frac(world / 8));
        float kerb = step(.48, frac(world / 3.5));
        col = sand;
        if (a < 1.09)
            col = lerp(float3(.99, .88, .70), float3(.81, .185, .075), kerb);
        if (a < 1)
            col = asphalt;
        if (a > .945 && a < .963)
            col = float3(.94, .92, .84);
        if (a < .014 && stripe > .5)
            col = float3(1, .76, .22);
        col = lerp(col, float3(.94, .64, .44), exp(-depth * 52) * .72);
    }
    // Sandstone outcrops at fixed world positions give the empty shoulder scale.
    [unroll] for (int rock = 4; rock >= 0; rock--) {
        float group = floor(distance / 57) + rock;
        float z = 19 + (rock + 1) * 57 - fmod(distance, 57);
        float dep = 6.4 / z;
        float side = fmod(group, 2) < 1 ? -1 : 1;
        float2 foot = float2(curve * .25 * pow(1 - dep, 2) - lane * dep * .46 +
                                 side * dep * (2.3 + .6 * abs(sin(group))),
                             horizon + dep * (1 - horizon));
        float2 local = (float2(x, uv.y) - foot) / dep;
        float mass = abs(local.x) * .47;
        float height = .48 - mass + .18 * ridge(local.x * 2 + group * 1.7, 17);
        height = floor(max(0, height) * 12) * .083;
        if (local.y < 0 && -local.y < height) {
            float layer = frac(-local.y * 17 + .065 * sin(local.x * 19));
            float light = saturate(.62 - local.x * .29 + .18 * ridge(local.x * 8 + group, 16));
            float3 stone = lerp(float3(.36, .13, .15), float3(.81, .34, .17), light);
            col = lerp(stone * (.87 + .13 * smoothstep(.02, .24, layer)), float3(.92, .52, .32),
                       exp(-dep * 14) * .52);
        }
    }
    // Reflector poles project from fixed positions alongside the road.
    [unroll] for (int i = 6; i >= 0; i--) {
        float z = 8 + (i + 1) * 14 - fmod(distance, 14);
        float dep = 6.4 / z, base = horizon + dep * (1 - horizon);
        float middle = curve * .25 * pow(1 - dep, 2) - lane * dep * .46;
        float2 pole = float2(abs(x - middle) - 1.12 * dep, uv.y - base);
        float shape = fillShape(box(pole + float2(0, dep * .075), float2(dep * .012, dep * .075)));
        float3 paint = pole.y < -.10 * dep ? float3(.78, .34, .17) : float3(.91, .85, .69);
        col = lerp(col, paint, shape);
    }
    // Rear silhouette of the time-trial coupe.
    float2 car = float2(x - secondary.x * .012, uv.y - .765);
    float2 body = float2(abs(car.x), car.y);
    col *= 1 - .53 * exp(-pow(car.x / .177, 2) - pow((car.y - .071) / .024, 2));
    col = lerp(col, float3(.075, .082, .08),
               fillShape(rounded(float2(body.x - .137, body.y - .044), float2(.032, .033), .007)));
    float width =
        .161 - .056 * saturate((-car.y - .009) / .083) - .01 * saturate((car.y - .033) / .03);
    float shell = fillShape(rounded(car, float2(width, .066), .011));
    float upper = fillShape(rounded(car + float2(0, .046), float2(width - .01, .047), .011));
    float3 paint =
        lerp(float3(.36, .046, .033), float3(1, .31, .075), saturate((.066 - car.y) * 10.5));
    paint += float3(.17, .16, .10) * exp(-pow((car.y + .022 + car.x * .05) / .008, 2));
    paint *= 1 - .20 * pow(saturate(body.x / width), 5);
    col = lerp(col, paint, max(shell, upper));
    float glass = rounded(car + float2(0, .054), float2(width - .020, .027), .004);
    col = lerp(col, float3(.08, .12, .13), fillShape(glass - .002));
    float3 window = lerp(float3(.065, .11, .18), float3(.41, .60, .74),
                         saturate((-car.y - .024) * 15 + car.x * .95));
    window += .055 * smoothstep(-.01, .02, car.x + car.y * .75);
    window *= .90 + .10 * smoothstep(.03, .22, frac(car.y * 115));
    col = lerp(col, window, fillShape(glass));
    float2 mirror = float2(body.x - .142, car.y + .042);
    col = lerp(col, paint * .78, fillShape(rounded(mirror, float2(.017, .007), .004)));
    float fascia = rounded(car - float2(0, .016), float2(.152, .014), .005);
    col = lerp(col, float3(.17, .12, .10), fillShape(fascia));
    float tail = rounded(float2(body.x - .100, car.y - .016), float2(.043, .008), .003);
    float rib = smoothstep(.04, .18, frac(body.x * 185));
    float3 lamp = lerp(float3(.49, .077, .047), float3(.98, .24, .11), rib);
    if (body.x > .135)
        lamp = lerp(float3(.85, .38, .12), float3(.96, .58, .20), rib);
    col = lerp(col, lamp, fillShape(tail));
    float bumper = rounded(car - float2(0, .048), float2(.153, .011), .005);
    col = lerp(col, float3(.105, .123, .124) + .12 * exp(-pow((car.y - .043) / .003, 2)),
               fillShape(bumper));
    col = lerp(col, float3(.39, .39, .34),
               fillShape(rounded(car - float2(0, .034), float2(.03, .010), .003)));
    col = lerp(col, float3(.91, .84, .63),
               fillShape(rounded(car - float2(0, .034), float2(.027, .007), .002)));
    if (body.x < .018 && abs(car.y - .034) < .003 && frac(car.x * 760) > .43)
        col = float3(.29, .27, .23);
    float pipe = length(float2(body.x - .111, (car.y - .060) * 1.7));
    col = lerp(col, float3(.61, .61, .56), fillShape(pipe - .009));
    col = lerp(col, float3(.08, .09, .09), fillShape(pipe - .0065));
    col = lerp(col, float3(.13, .16, .16),
               fillShape(rounded(car + float2(0, .011), float2(.167, .004), .002)));
    col = lerp(col, float3(.79, .42, .22),
               fillShape(rounded(car + float2(0, .014), float2(.166, .0015), .001)));
    return float4(saturate(col), 1);
}
