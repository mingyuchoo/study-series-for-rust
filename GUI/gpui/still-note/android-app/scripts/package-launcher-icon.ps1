param()
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
$source = [System.Drawing.Bitmap]::new((Join-Path $root 'docs/assets/stillnote-icon-source.png'))
try {
    # Trim only transparent padding, then center the generated artwork in the safe zone.
    $left = $source.Width
    $top = $source.Height
    $right = 0
    $bottom = 0
    for ($y = 0; $y -lt $source.Height; $y++) {
        for ($x = 0; $x -lt $source.Width; $x++) {
            if ($source.GetPixel($x, $y).A -gt 8) {
                $left = [Math]::Min($left, $x)
                $top = [Math]::Min($top, $y)
                $right = [Math]::Max($right, $x)
                $bottom = [Math]::Max($bottom, $y)
            }
        }
    }
    if ($left -gt $right -or $top -gt $bottom) { throw 'Icon source is empty.' }
    $bounds = [System.Drawing.RectangleF]::new($left, $top, $right - $left + 1, $bottom - $top + 1)
    $fit = [Math]::Min(50.0 / $bounds.Width, 56.0 / $bounds.Height)
    $width = $bounds.Width * $fit
    $height = $bounds.Height * $fit
    foreach ($entry in @(@('mdpi', 108), @('hdpi', 162), @('xhdpi', 216), @('xxhdpi', 324), @('xxxhdpi', 432))) {
        $size = [int]$entry[1]
        $dir = Join-Path $root "app/res/drawable-$($entry[0])"
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $bitmap = [System.Drawing.Bitmap]::new($size, $size)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.Clear([System.Drawing.Color]::Transparent)
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $scale = $size / 108.0
            $dest = [System.Drawing.RectangleF]::new((108 - $width) / 2 * $scale, (108 - $height) / 2 * $scale, $width * $scale, $height * $scale)
            $graphics.DrawImage($source, $dest, $bounds, [System.Drawing.GraphicsUnit]::Pixel)
            $bitmap.Save((Join-Path $dir 'ic_launcher_foreground.png'), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
    }
    # Preview the 72dp launcher viewport; Android applies the actual device mask.
    $preview = [System.Drawing.Bitmap]::new(512, 512)
    $graphics = [System.Drawing.Graphics]::FromImage($preview)
    try {
        $graphics.Clear([System.Drawing.Color]::FromArgb(255, 10, 10, 10))
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $scale = 512 / 72.0
        $dest = [System.Drawing.RectangleF]::new((72 - $width) / 2 * $scale, (72 - $height) / 2 * $scale, $width * $scale, $height * $scale)
        $graphics.DrawImage($source, $dest, $bounds, [System.Drawing.GraphicsUnit]::Pixel)
        $preview.Save((Join-Path $root 'docs/assets/stillnote-icon-preview.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $preview.Dispose()
    }
    Write-Output "Packaged launcher artwork: $([Math]::Round($width, 2)) x $([Math]::Round($height, 2)) dp inside 108dp layers."
} finally {
    $source.Dispose()
}
