# 生成 1024x1024 应用图标源图（Lexica "L"）。
# 用法：powershell -File scripts/make-icon.ps1
# 生成后运行：npm run tauri icon ..\app-icon.png（在 src-tauri 下）产出全套平台图标。
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$size = 1024
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias
$g.Clear([System.Drawing.Color]::Transparent)

# 圆角方块底（品牌赭色 #8a5a2b）
$m = 64
$r = 176
$w = $size - 2 * $m
$h = $w
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$path.AddArc($m, $m, $r, $r, 180, 90)
$path.AddArc($m + $w - $r, $m, $r, $r, 270, 90)
$path.AddArc($m + $w - $r, $m + $h - $r, $r, $r, 0, 90)
$path.AddArc($m, $m + $h - $r, $r, $r, 90, 90)
$path.CloseFigure()
$bg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 138, 90, 43))
$g.FillPath($bg, $path)

# 衬线字母 L（与产品正文衬线基调一致）
$font = New-Object System.Drawing.Font('Georgia', 500, [System.Drawing.FontStyle]::Bold)
$fg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = [System.Drawing.StringAlignment]::Center
$fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
$rect = New-Object System.Drawing.RectangleF(0, -30, $size, $size)
$g.DrawString('L', $font, $fg, $rect, $fmt)

$out = Join-Path (Split-Path $PSScriptRoot -Parent) 'app-icon.png'
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output "written: $out"
