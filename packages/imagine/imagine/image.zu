#!-- part of the imagine module

import _imagine

import .resource { ImageResource }


/**
 * The Image class is allows creating and opening of imnages in 
 * any of the supported formats which includes `JPEG`, `PNG`, 
 * `GIF`, `TIFF`, `BMP`, `WBMP`, `TGA`, `WEBP`, `AVIF`.
 */
class Image {

  /**
   * Creates a palette-based image (up to 256 colors) or a truecolor 
   * image (millions of colors) when `use_true_colors` is set to true.
   * 
   * @param number width
   * @param number height
   * @param bool? use_true_colors
   * @returns [[imagine.ImageResource]]
   */
  static new(width, height, use_true_colors) {
    if use_true_colors == nil {
      use_true_colors = true
    }

    if !is_number(width)
      raise TypeError('number expected at width expected, ${typeof(width)} given')
    if !is_number(height)
      raise TypeError('number expected at height expected, ${typeof(width)} given')
    if !is_bool(use_true_colors)
      raise TypeError('boolean expected at use_true_colors expected, ${typeof(use_true_colors)} given')

    return ImageResource(_imagine.new(width, height, use_true_colors))
  }

  /**
   * Creates an image from a PNG file. Truecolor PNG stays truecolor; 
   * palette PNG stays palette-based.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_png(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.frompng(src))
  }

  /**
   * Creates an image from a JPEG file.
   * JPEG is always truecolor.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_jpeg(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromjpeg(src))
  }

  /**
   * Creates an image from a GIF file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_gif(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromgif(src))
  }

  /**
   * Creates an image from a BMP file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_bmp(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.frombmp(src))
  }

  /**
   * Creates an image from a WBMP file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_wbmp(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromwbmp(src))
  }

  /**
   * Creates an image from a TGA file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_tga(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromtga(src))
  }

  /**
   * Creates an image from a TIFF file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_tiff(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromtiff(src))
  }

  /**
   * Creates an image from a WEBP file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_webp(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromwebp(src))
  }

  /**
   * Creates an image from a AVIF file.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_avif(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_string(src) {
      src = file(src)
    }

    if !src.exists() {
      raise Exception('file not found')
    }

    if src.mode().index_of('r') == -1 {
      raise Exception('file not readable')
    }

    return ImageResource(_imagine.fromavif(src))
  }

  /**
   * Creates an image from any supported image file.
   * As long as the file type is supported by Imagine,
   * the file type will automatically be detected.
   * 
   * @param string|file src
   * @returns [[imagine.ImageResource]]
   */
  static from_file(src) {
    if !is_string(src) and !is_file(src) 
      raise TypeError('string path or file expected, ${typeof(src)} given')

    if is_file(src) {
      src = file.abs_path()
    }

    return ImageResource(_imagine.fromfile(src))
  }
}
