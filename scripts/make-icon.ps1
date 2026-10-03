# 生成 Loomy 账号管理器图标（1024x1024，**透明背景**）
#
# 设计要点：
#   1. 透明背景（用户要求）—— 不要深色方块
#   2. 圆角方形徽章而不是圆形 —— 圆形像"通用头像"，圆角方形是现代应用图标的语言
#   3. 对角渐变 + 顶部柔光，柔光用 Clamp 防止出现重复色带
#   4. 字形用**矢量路径**画（矩形拼），不用字体 —— 字体可能 fallback，跨机器结果不一致
#   5. 包围盒居中，不加主观偏移
#
# 踩过的两个坑（都在这次渲染里出现过）：
#   - 柔光层用 LinearGradientBrush 直接 FillRectangle 整张画布时，默认
#     WrapMode=Tile 会让渐变**重复**，在终止位置形成一条硬色带。
#     修法：WrapMode 设为 Clamp，且填充区域与渐变区域完全一致。
#   - 字形占徽章比例过大（>60%）会显得拥挤。留白量本身是设计的一部分。

Add-Type -AssemblyName System.Drawing

$S = 1024
$bmp = New-Object System.Drawing.Bitmap $S, $S, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)

$g.SmoothingMode      = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.InterpolationMode  = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.PixelOffsetMode    = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
$g.Clear([System.Drawing.Color]::Transparent)   # ← 透明底

function New-RoundedRect([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $p.AddArc($x,           $y,           $d, $d, 180, 90)
  $p.AddArc($x + $w - $d, $y,           $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d,   0, 90)
  $p.AddArc($x,           $y + $h - $d, $d, $d,  90, 90)
  $p.CloseFigure()
  return $p
}

# ── 徽章 ────────────────────────────────────────────────────────────────
$M  = 72.0                       # 边距（留白）
$BW = $S - $M * 2                # 880
$badge = New-RoundedRect $M $M $BW $BW ($BW * 0.225)

# 对角渐变：亮天蓝 → 深靛蓝（与界面主色 #3b82f6 同族）
$c1 = [System.Drawing.Color]::FromArgb(255,  96, 165, 250)   # #60A5FA
$c2 = [System.Drawing.Color]::FromArgb(255,  67,  56, 202)   # #4338CA
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
  (New-Object System.Drawing.PointF $M, $M),
  (New-Object System.Drawing.PointF ($M + $BW), ($M + $BW)),
  $c1, $c2)
$g.FillPath($grad, $badge)

# ── 顶部柔光 ────────────────────────────────────────────────────────────
# 第一版出现了一条横向硬色带，原因有两点，都要避开：
#
#   ① LinearGradientBrush 的默认 WrapMode 是 Tile —— 画布比渐变区域高时，
#      渐变会**平铺重复**，在终点位置形成一条边。
#   ② `WrapMode = Clamp` 在这里**不可用**：那个枚举属于 TextureBrush，
#      给 LinearGradientBrush 赋值会抛 "Parameter is not valid."
#
# 正确修法：不要覆盖整张画布，而是让 **填充区域与渐变区域完全一致** ——
# 渐变在区域边界结束，没有多余空间可平铺，自然就没有色带。
$sheenTop = [System.Drawing.Color]::FromArgb(58, 255, 255, 255)   # 白 ~23%
$sheenEnd = [System.Drawing.Color]::FromArgb( 0, 255, 255, 255)   # 全透明
$sheenH   = $BW * 0.62
$sheenRect = New-Object System.Drawing.RectangleF($M, $M, $BW, $sheenH)
$sheen = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
  $sheenRect, $sheenTop, $sheenEnd,
  [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
$g.SetClip($badge)
$g.FillRectangle($sheen, $sheenRect)   # ← 填充区域 == 渐变区域，不会平铺
$g.ResetClip()

# ── 字形：几何 L ────────────────────────────────────────────────────────
# 占徽章高度约 48% —— 留白够了才不拥挤
$strokeW = 108.0
$stemH   = $BW * 0.48            # 422
$footW   = $BW * 0.30            # 264

$bbW = $footW
$bbH = $stemH
$bx  = ($S - $bbW) / 2           # 包围盒居中，不加主观偏移
$by  = ($S - $bbH) / 2

$white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)

# 竖笔：顶端圆角小一点（看起来像字形而不是胶囊）
$g.FillPath($white, (New-RoundedRect $bx $by $strokeW $bbH 16))
# 横笔：与竖笔底端齐平
$g.FillPath($white, (New-RoundedRect $bx ($by + $bbH - $strokeW) $footW $strokeW 16))

$g.Dispose()

$out = "D:\temp\icon-src.png"
New-Item -ItemType Directory -Force -Path "D:\temp" | Out-Null
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

# ── 验证 ────────────────────────────────────────────────────────────────
$chk = New-Object System.Drawing.Bitmap $out
"  尺寸 $($chk.Width)x$($chk.Height)  $($chk.PixelFormat)"

# 1) 四角必须完全透明
$corners = @(@(1,1), @(1022,1), @(1,1022), @(1022,1022))
$allClear = $true
foreach ($pt in $corners) {
  $px = $chk.GetPixel($pt[0], $pt[1])
  if ($px.A -ne 0) { $allClear = $false; "    ✗ 角($($pt[0]),$($pt[1])) A=$($px.A) 不是透明" }
}
"  四角透明: $(if($allClear){'✓'}else{'✗'})"

# 2) 沿**背景**中轴自上而下取样，确认没有硬色带。
#
#    ⚠ 不能取 x=512：那是字形竖笔所在的位置，穿过白色 L 会得到一个巨大的
#    "亮度跳变"，把真正的色带问题掩盖掉（第一版就是这么误报的）。
#    字形包围盒在 x∈[380,644]，所以取 x=200 是纯背景。
$prev = $null
$maxJump = 0
$jumpAt = 0
for ($y = 90; $y -le 940; $y += 6) {
  $px = $chk.GetPixel(200, $y)
  if ($px.A -eq 0) { $prev = $null; continue }   # 出徽章了
  $lum = 0.299*$px.R + 0.587*$px.G + 0.114*$px.B
  if ($prev -ne $null) {
    $jump = [Math]::Abs($lum - $prev)
    if ($jump -gt $maxJump) { $maxJump = $jump; $jumpAt = $y }
  }
  $prev = $lum
}
"  背景纵向最大亮度跳变: $([math]::Round($maxJump,1)) (在 y=$jumpAt)  $(if($maxJump -lt 8){'✓ 无硬色带'}else{'✗ 可能有色带'})"

# 3) 确认字形确实是白色（防止填充/裁剪出错）
$gx = $chk.GetPixel(400, 500)
"  字形中心像素: A=$($gx.A) R=$($gx.R) G=$($gx.G) B=$($gx.B)  $(if($gx.R -gt 240 -and $gx.G -gt 240){'✓ 白色'}else{'✗ 不是白色'})"

$chk.Dispose()
"  已生成 $out  ($([math]::Round((Get-Item $out).Length/1KB,1)) KB)"
