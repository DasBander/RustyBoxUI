cbuffer Parameters : register(b0) {
    float4 viewport;
    float4 primary;
    float4 secondary;
};
float4 vertex(uint id : SV_VertexID) : SV_POSITION {
    float2 uv = float2((id << 1) & 2, id & 2);
    return float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
}
