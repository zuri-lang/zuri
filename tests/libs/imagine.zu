import imagine {*}
import os

catch {
  if !os.dir_exists('./tmp')
    os.create_dir('./tmp')

  Image.new(130, 20, true).use(@(im) {
    im.save_alpha()

    var bg_color = im.allocate_color(0, 0, 0, 127)
    var fore_color = im.allocate_color(233, 14, 91)

    im.fill(0, 0, bg_color)
    im.string(5, 2, 'A simple text string', FONT_REGULAR, fore_color)

    im.export_png('./tmp/image1.png')
  })

  Image.new(100, 100).use(@(im) {
    var black = im.allocate_color(0, 0, 0)
    var white = im.allocate_color(255, 255, 255)

    im.fill(0, 0, white)
    im.filled_arc(50, 50, 98, 98, 0, 204, black)

    im.export_webp('./tmp/image2.webp')
  })

  Image.new(640, 640, true).use(@(im) {
    var bg_color = im.allocate_color(0, 0, 0, 127)
    im.fill(0, 0, bg_color)

    # Commenting this out till we find a way to fix WebP on all supported OS.
    Image.from_png('./tmp/image1.png').use(@(im2) {
      var meta = im2.meta()

      im.copy_resized(im2, 0, 0, 0, 0, 640, 640, meta.width, meta.height)
    })

    im.export_jpeg('./tmp/image3.jpg')
  })
} as e

if e {
  echo ''
  echo 'IMAGINE TEST ERROR:'
  echo '======================================================'
  echo e.message
  echo e.stacktrace
}
