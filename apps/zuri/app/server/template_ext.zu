import markdown
import highlight { highlight }
import .util
import log


var md = markdown({
  html: true,
  highlight: highlight(),
})

def template_ext() {
  return {
    plus_one: @( t ) {
      return t + 1
    },
    format_number: @( t ) {
      return util.format_number(t)
    },
    strip_line: @( t ) {
      return t.replace('\n', '\\n')
    },
    sort_name: @( t ) {
      return t == 'name'
    },
    sort_download: @( t ) {
      return t == 'downloads'
    },
    sort_created: @( t ) {
      return t == 'created_at'
    },
    br: @( t ) {
      return t.replace('\n', '<br>')
    },
    draw: @( t ) {
      catch {
        return md.render(t)
      } as e

      if e {
        log.exception(e)
        return t
      }
    },
    can_revert: @( versions ) {
      return versions.length() > 1
    },
    no_ext: @( t ) {
      return t.split('.')[0]
    },
    file_title: @( t ) {
      var x = t.split('.')[0].replace('-', ' ')
      return x[0].upper() + x[1,]
    },
  }
}