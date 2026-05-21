/**
 * @module template
 * 
 * The template module contains classes and functions for working with Blade's `wire` 
 * templates. Wire templating is an extensible template system built on standard HTML5. 
 * In fact, any valid HTML5 document is also a valid Wire template. Wire builds atop 
 * the existing HTML5 language to provide template support for Web development in Blade 
 * and makes extensive use of HTML attributes for condition and looping.
 * 
 * Wire templates allow creation of custom HTML elements (such as the builtin `<include />` 
 * tag that allows for inlining other template files in a template file.). It's a simple 
 * yet effective system of dynamic programming and the flow altogether feels like writing 
 * in a frontend framework such as VueJS.
 * 
 * ### Basic Usage
 * 
 * ```blade
 * import template
 * 
 * var tpl = template()
 * echo tpl.render('mytemplate')
 * ```
 * 
 * Or to render from a string
 * 
 * ```blade
 * echo tpl.render_string('<p>{{ name }}</p>', {name: 'Hello World'})
 * ```
 * 
 * The last example above will render the following:
 * 
 * ```wire
 * <p>Hello World</p>
 * ```
 * 
 * ### Comments
 * 
 * Wire inherits HTMLs comments using the same syntax `<!-- ... -->`. It is important to note 
 * that Wire does not render comments in the output HTML. For web applications, this helps you 
 * to write server side comments in your Wire files without it getting to the frontend since 
 * Wire is backend code.
 * 
 * For example, the code below should return an empty string.
 * 
 * ```blade
 * tpl.render_string('<!-- HTML or Wire comment? -->')
 * ```
 * 
 * ### Variables
 * 
 * Variables in Wire templates are names surrounded by `{{` and `}}` pair. For example, to print 
 * the value of a variable _myvar_ passed into [[Template.render]] or [[Template.render_string]] 
 * in the template, you can do it like this.
 * 
 * ```wire
 * <div>{{ myvar }}</div>
 * ```
 * 
 * > **NOTE:** 
 * > - The `<div>` and `</div>` surround the variable and are not part of the variable.
 * > - The spaces around the variable are just formatting and are not required.
 * 
 * The exception to this is when passing a variable to a reserved attribute such as `x-key`. 
 * In this case, you'll need to omit the surrounding braces. 
 * 
 * For example:
 * 
 * ```wire
 * <div x-for="myvar" x-key="mykey"></div>
 * ```
 * 
 * As shown in the example above, variables can occur anywhere in a Wire template including in 
 * element attributes.
 * 
 * To print the exact characters `{{ myvar }}` if that's what you actually mean and stop if from 
 * being interpreted as a variable, you'll need to escape the first `{` with the percent sign `%`. 
 * 
 * For example:
 * 
 * ```wire
 * <div>%{{ myvar }}</>
 * ```
 * 
 * The example above will return the following:
 * 
 * ```wire
 * <div>{{ myvar }}</div>
 * ```
 * 
 * ### Expressions and Modifiers
 * 
 * Expressions in Wire are a feature that allows modification of value directly in the template. 
 * An __Expression__ is value that has been modified by passing it through a functions called 
 * __Modifiers__ using the pipe (`|`) character. Wire comes with a lot of built-in functions for 
 * creating expressions and they are all described at below.
 * 
 * For example:
 * 
 * ```wire
 * <div>{{ name|length }}</div>
 * ```
 * 
 * In the example above, the name variable was expressed as its length by passing it into through 
 * the _length_ modifier function. If _name_ contains the value `John Doe`, then the value printed 
 * will be `8`.
 * 
 * The built-in modifiers are documented under [Template Functions](#template-functions).
 * 
 * Some expression modifiers require that a value is passed. To pass value to a modifier, use the 
 * equal (`=`) sign. For example:
 * 
 * ```wire
 * {{ name|is='Jane' }}
 * ```
 * 
 * In this example, `Jane` is a string therefore it is quoted. You can also pass the name of another 
 * variable, a number or any of the constants `true`, `false`, and `nil` directly without the quotes.
 * 
 * For example:
 * 
 * ```wire
 * {{ age|is=30.5 }}
 * ```
 * 
 * ### If... and If not...
 * 
 * Wire implements conditionals via the `x-if` and `x-not` attribute that can be attached to any HTML 
 * element. This attributes are never returned in the compiled HTML output and decides whether an
 * element will be printed or not. The `x-if` attribute evaluates a variable or expression and will only
 * print the element to which it is attached and its children if the result of the expression or variable 
 * evaluation returns a value that is boolean `true` in Blade. The `x-not` attribute does the reverse of
 * this (i.e. it only prints if the evaluation returns Blade boolean `false`).
 * 
 * ```blade
 * tpl.render_string('<div x-if="name">Hello</div>')
 * ```
 * 
 * The example above will return an empty string since the variable name was never declared. However, the 
 * reverse is the case if the attribute was `x-not`. 
 * 
 * For example:
 * 
 * ```blade
 * tpl.render_string('<div x-not="name">Hello</div>')
 * ```
 * 
 * The example above will return `<div>Hello</div>`.
 * 
 * ### Loops
 * 
 * Wire templates support for loops is enabled via the `x-for` attribute that can be applied to any 
 * element. When the `x-for` attribute is present on an element, it must declare a corresponding `x-value` 
 * attribute that defines the name of the value variable within the loop. An optional `x-key` attribute 
 * may also be defined to define a variable name that will contain the value of the iteration index/key.
 * 
 * The _x-for_ attribute must declare a variable or expression that evaluates into an iterable (such as a 
 * string, list, dictionary etc.).
 * 
 * For example:
 * 
 * ```blade
 * tpl.render_string('<div x-for="data">Ok</div>', {data: 0..3})
 * ```
 * 
 * The code above will return the following:
 * 
 * ```wire
 * <div>Ok</div><div>Ok</div><div>Ok</div>
 * ```
 * 
 * > The original `<div>` was replicated three times without the `x-for` attribute. 
 * > __Wire attributes are applied to an element and their children not the children only.__
 * 
 * Here is an example using the `x-value` attribute to print the items in a list.
 * 
 * ```blade
 * tpl.render_string('<div x-for="data" x-value="val">{{ val }}</div>', {data: ['apple', 'mango']})
 * ```
 * 
 * The code above return
 * 
 * ```wire
 * <div>apple</div><div>mango</div>
 * ```
 * 
 * We could decide to print the index as well by adding a new variable using the `x-key` attribute.
 * 
 * ```blade
 * tpl.render_string('<div x-for="data" x-value="val" x-key="key">
 *   <span>{{ key }}</span>
 *   <span>{{ value }}</span>
 * </div>', {data: ['apple', 'mango']})
 * ```
 * 
 * Which will output
 * 
 * ```wire
 * <div>
 *   <span>0</span>
 *   <span>apple</span>
 * </div><div>
 *   <span>1</span>
 *   <span>mango</span>
 * </div>
 * ```
 * 
 * ### Wiring templates
 * 
 * While most of the examples here use the `render_string()` function to give a practical approach 
 * to learning Wire templates, the `render()` function which allows rendering Wire templates from 
 * files is a more powerful and conventional method of using Wire templates. Not only because they 
 * allow loading templates from files, but also because they allow including other template files in 
 * a template file via the builtin `<include />` tag. The `include` tag allows wiring multiple Wires 
 * together to create a comprehensive UI layout hierarchy and is quite intuitive to use. 
 * 
 * Let's consider a simple use case: 
 * 
 * In a website for a client all pages UTF-8 enabled and are mobile first. This leaves room for a set 
 * of `<meta>` tags that will need to be on every page of the website and in practice it will soon 
 * become burdensome to have to keep repeating the `meta` tags across all page templates. To reduce
 * this code duplication, we can have a file located at the template root directory (See 
 * [[Template.set_root]]) that contains all shared `meta` tags as shown in the sample below and include 
 * this file in every other template.
 * 
 * ```wire
 * <!-- templates/meta.html -->
 * <meta charset="utf-8">
 * <meta http-equiv="X-UA-Compatible" content="IE=edge">
 * <meta name="viewport" content="width=device-width, initial-scale=1.0, shrink-to-fit=no">
 * ```
 * 
 * This template can the be imported in another file with the `include` tag.
 * 
 * ```wire
 * <!-- templates/layout.html -->
 * <include path="meta.html" />
 * ```
 * 
 * The `include` tag has only one attribute which is always required and that is the `path` attribute. 
 * This attribute allows us to specify the path to a Wire template (or any HTML file for that matter) 
 * that will be rendered in the position the `include` tag currently occupies. Take note that in the 
 * example above the `path` argument did not start with "templates/". This is because when decoding the 
 * include path, the library first searches for files in the template root directory and if a matching 
 * file is found, that file will be rendered. If none is found, it will interpret the path as a relative 
 * path first then as an absolute path if no match is found. 
 * 
 * See [[Template.render]] for more information.
 * 
 * ### Custom Modifiers
 * 
 * Apart from the built-in value modifiers, Wire templates allow you to add custom modifiers in a 
 * simple manner by registering them with the `register_function()` method. The example below 
 * shows an example custom modifier __reverse__ that reverses the original value as a string.
 * 
 * ```blade
 * tpl.register_function('reverse', @(value) {
 *   return ''.join(to_list(value).reverse())
 * })
 * ```
 * 
 * The modifier __reverse__ can then be used in a Wire template like this:
 * 
 * ```blade
 * tpl.render_string('<div>{{ fruit|reverse }}</div>', {fruit: 'mango'})
 * ```
 * 
 * And the output HTML from the above code will be
 * 
 * ```wire
 * <div>ognam</div>
 * ```
 * 
 * Modifier functions can also take a second argument which will receive any argument passed to the
 * modifier. This is best expressed with an example.
 * 
 * ```blade
 * tpl.register_function('reverse_weird', @(value, arg) {
 *   return '${arg}: ' + ''.join(to_list(value).reverse())
 * })
 * ```
 * 
 * This above modifier expects an argument that will be used to append the return string. While we 
 * acknowledge that this function/modifier is weird, it shows clearly how to create a modifier that 
 * takes an argument.
 * 
 * The code below shows how to pass an argument into the `reverse_weird` modifier from a template.
 * 
 * ```wire
 * <p>{{ fruit|reverse_weird='Reversed' }}</p>
 * ```
 * 
 * Yes I know. It's weird. But if we passed in the same argument as the last, the output will be
 * 
 * ```wire
 * <p>Reversed: ognam</p>
 * ```
 * 
 * Like regular Blade code, the argument will be `nil` if not passed and this is 
 * important information if you intend to leverage this for a library that will be used by other 
 * people. 
 * 
 * If we remove the argument to the modifier in the template above and simply call 
 * `fruit|reverse_weird`, the result will look like this:
 * 
 * ```wire
 * <p>: ognam</p>
 * ```
 * 
 * ### Custom Tags
 * 
 * As with custom modifiers the template library allows you to create and process custom tags. 
 * An example of a custom tag is the `<include />` tag previously discussed. To declare a custom 
 * element and its behavior, you need to create a function that accepts two arguments and 
 * register it with the `register_element()` method. When your custom element is matched in a 
 * template, the registered function will be called with an instance of [[template.Template]] in the first 
 * argument and the {{html}} decoded template as the second argument. Your function must then 
 * return a string representing the processed tag or a valid HTML element Blade representation as 
 * defined by the {{html}} module. 
 * 
 * > NOTE: It's more memory efficient to modify and return the same element when returning an HTML
 *    representation.
 * 
 * The example below defines a custom tag _`link`_ that will always be rendered as an anchor 
 * (`<a>`) element with the class `link`.
 * 
 * ```blade
 * tpl.register_element('link', @(this, el) {
 *   return '<a href="${this.attr(el, 'href').value}">${this.attr(el, 'text').value}</a>'
 * })
 * ```
 * 
 * The simple tag defined above allows us to process the `<link />` tag in a Wire template. 
 * For example,
 * 
 * ```wire
 * <link href="bladelang.com" text="Blade Website" />
 * ```
 * 
 * The Wire template above will cause the following to be rendered.
 * 
 * ```wire
 * <a href="bladelang.com">Blade Website</a>
 * ```
 * 
 * Below is a more complex example that returns an HTML representation in Blade instead of a string.
 * 
 * ```blade
 * tpl.register_element('link', @(this, el) {
 *   return {
 *     type: 'element',
 *     name: 'a',
 *     attributes: [
 *       { name: 'href', value: this.attr(el, 'href').value }
 *     ],
 *     children: [
 *       { type: 'text', content: this.attr(el, 'text').value }
 *     ]
 *   }
 * })
 * ```
 * 
 * Both code achieve the same thing. However, the later format allows for a more flexible and programmatic 
 * output that the former and is the recommended approach wherever possible.
 * 
 * ### Template Functions
 * 
 * Template functions in Wire are simply modifiers that do not process any value nor accept any argument 
 * (i.e. stand-alone modifiers) and are created in the same way as we create modifiers. However, they are 
 * invoked quite differently. To invoke a template function, you need to wrap them in a `{!` and `!}` pair. 
 * 
 * For example, consider the following template function defined to return the base url of a website.
 * 
 * ```blade
 * tpl.register_function('base_url', @{
 *   return  'https://localhost:8000'
 * })
 * ```
 * 
 * The function can be invoked as follows:
 * 
 * ```blade
 * tpl.render_string('{! base_url !}')
 * ```
 * 
 * The example above will return `https://localhost:8000`.
 * 
 * Like with the `{{` and `}}` pair for variables, if you really intend to write the `{!` and `!}` pair, 
 * you'll need to escape the first `{` with a `%` sign. For example, `%%{! name !}` will render as
 * `{! name !}` without processing.
 */

import os
import json
import html
import reflect
import .functions as fns
import .constants

# create the default html config
var void_tags = html.void_tags
void_tags.append('include')

var _default_html_config = {
  with_position: true,
  void_tags,
}

/**
 * Returns the attribute of the given name from the given HTML element. If the attribute is not 
 * found, `nil` is returned.
 * 
 * @param dict element
 * @param string name
 * @returns dict?
 */
def attr(element, name) {

  # Ensure that the element is a valid HTML element as per the {{html}} module since we will be working 
  # with them and we want to avoid unexpected errors. Also ensure that the name is a string.
  if !(is_dict(element) and !element.contains('type') and element.type == 'element') {
    raise Exception('html element expected')
  }

  if !is_string(name) {
    raise Exception('string expected in argument 2 (name)')
  }

  # Normalize:
  # 
  # Convert attribute name to lowercase case since HTML attributes are case insensitive and the HTML 
  # parser decodes all attributes to lowercase anyway.
  name = name.lower()

  for attr in element.attributes {
    if attr.name.lower() == name 
      return attr
  }

  return nil
}

/**
 * Template string and file processing class.
 * 
 * ##### Usage
 * 
 * You can render templates directly from strings
 * 
 * ```blade
 * import template
 * var tpl = template()
 * 
 * tpl.render_string('{{ name }}', {name: 'John Doe'})
 * ```
 * 
 * Or from files located in your defined root directory. See [[Template.set_root]]
 * 
 * ```blade
 * tpl.render('my_template', {name: 'John Doe'})
 * ```
 * 
 * You can enable initialize your templates with the auto_init option to allow 
 * [[Template]] create the root directory if it does not exist. The default root 
 * directory is a directory "`templates`" in the current working directory.
 * 
 * For example,
 * 
 * ```blade
 * var tpl = template(true)
 * 
 * # Optionally set the root directory to another directory.
 * tpl.set_root('/my/custom/path')
 * ```
 * 
 * The root directory will become the root search path for the `<include />` tag.
 * 
 * The default extension for a template file is the `.html` extension. This extension 
 * allows furnishes the interoperability between Blade's Wire templates and HTML5 since the
 * former is based on the later anyway and allows us to leverage the already near 
 * omnipresent support that HTML files have had over the years. This behavior can be 
 * changed using the [[Template.set_extension]] function to change the extension to any 
 * desired string.
 * 
 * For example,
 * 
 * ```blade
 * tpl.set_extension('.wire')
 * 
 * # render a template from file
 * tpl.render('welcome')
 * ```
 * 
 * This will cause [[Template.render]] to look for the file "`welcome.wire`" in the root 
 * directory and will return an error if the file could not be found and no file matches 
 * exactly "`welcome`" in the directory.
 */
class Template {

  # value modifier functions
  var _functions = fns.mapping.clone()

  # custom  elements
  var _elements = {}

  # root directory
  var _root_dir = constants.DEFAULT_ROOT_DIR

  # auto_init control
  var _auto_init = false

  # template file extension
  var _file_ext = constants.DEFAULT_EXT

  /**
   * The constructor of the Template class.
   * 
   * @param bool auto_init: A boolean flag to control whether template root 
   *    directory will be automatically created on [[Template.set_root]] or 
   *    [[Template.render]].
   * @constructor
   */
  Template(auto_init) {
    if auto_init != nil and !is_bool(auto_init)
      raise Exception('boolean expected in argument 1 (auto_init)')
    self._auto_init = auto_init == nil ? false : auto_init
  }

  _get_attrs(attrs) {
    return attrs.reduce(@(dict, attr) {
      dict.set(attr.name, attr.value)
      return dict
    }, {})
  }

  # remove comments and surrounding white space
  _strip(txt) {
    return txt.trim().replace(constants.COMMENT_RE, '')
  }

  _strip_attr(element, ...) {
    var attrs = __args__
    element.attributes = element.attributes.filter(@(el) {
      return !attrs.contains(el.name)
    })
  }

  _extract_var(variables, _var, error) {
    var var_split = _var.split('|')
    if var_split {
      var _vars = var_split[0].split('.')
      var real_var
  
      if variables.contains(_vars[0]) {
        if _vars.length() > 1 {
          var final_var = variables[_vars[0]]
          iter var i = 1; i < _vars.length(); i++ {
            if is_dict(final_var) {
              final_var = final_var.get(_vars[i].matches(constants.NUMBER_RE) ? 
                to_number(_vars[i]) : _vars[i], nil)
            } else if (is_list(final_var) or is_string(final_var)) and 
              _vars[i].matches(constants.NUMBER_RE) {
              final_var = final_var[to_number(_vars[i])]
            } else {
              error('could not resolve "${_var}" at "${_vars[i]}"')
            }
          }
  
          real_var = final_var
        } else {
          real_var = variables[_vars[0]]
        }
  
        if var_split.length() > 1 {
          iter var i = 1; i < var_split.length(); i++ {
            var fn = var_split[i].split('=')
            if self._functions.contains(fn[0]) {
              if fn.length() == 1 {
                real_var = self._functions[fn[0]](real_var)
              } else {
                var val = fn[1]
                if val.match(constants.QUOTE_VALUE_RE) {
                  real_var = self._functions[fn[0]](real_var, val[1,-1])
                } else {
                  var vval

                  if val == 'nil' vval = nil
                  else if val == 'true' vval = true
                  else if val == 'false' vval = false
                  else if val.match(constants.NUMBER_WITH_DECIMAL_RE) 
                    vval = to_number(val.match(constants.NUMBER_WITH_DECIMAL_RE)[0])
                  else vval = self._extract_var(variables, val, error)

                  real_var = self._functions[fn[0]](real_var, vval)
                }
              }
            } else {
              error('template function "${fn[0]}" not declared')
            }
          }
        }
  
        return real_var
      } else {
        # error('could not resolve "${_vars[0]}"')

        # Instead of returning an error, we'll return an empty string. This allows us to test 
        # for false, allows us to catch non-existing variables without an Exception and still
        # allow modifiers to be used with the non existing value. 
        # 
        # E.g. x-if="nonexistingvar|length" should still be false.
        return ''
      }
    } else {
      error('invalid variable "${_var}"')
    }
  
    return ''
  }

  _replace_funcs(content, error) {
    # prepare
    content = content.replace('%{!', '%{\x01!')
    # replace functions: {! fn !}
    # 
    # NOTE: This must come only just after variable replace as previous actions could generate or 
    # contain functions as well.
    var fn_vars = content.matches(constants.FUNCTION_RE)
    if fn_vars {
      # var_vars = json.decode(json.encode(fn_vars))
      iter var i = 0; i < fn_vars.fn.length(); i++ {
        var fn
        if (fn = self._functions.get(fn_vars.fn[i], nil)) and fn {
          content = content.replace(fn_vars[0][i], fn(), false)
        }
      }
    }
    
    # strip function escapes
    return content.replace('%{\x01!', '{!', false)
  }

  _replace_vars(content, variables, error) {
    # prepare
    content = content.replace('%{{', '%{\x01{')
    # replace variables: {{var_name}}
    # 
    # NOTE: This must come last as previous actions could generate or 
    # contain variables as well.
    var var_vars = content.matches(constants.VAR_RE)
    if var_vars {
      # var_vars = json.decode(json.encode(var_vars))
      iter var i = 0; i < var_vars.variable.length(); i++ {
        if var_vars[0][i] {
          content = content.replace(
            var_vars[0][i], 
            to_string(self._extract_var(variables, var_vars.variable[i], error)), 
            false
          )
        }
      }
    }
    
    # strip variable escapes
    return self._replace_funcs(content.replace('%{\x01{', '{{', false), error)
  }

  _get_template_content(path) {
    var template_path = os.join_paths(self._root_dir, path)
    if !template_path.match(constants.EXT_RE) template_path += constants.DEFAULT_EXT
    var fl = file(template_path)

    if fl.exists() {
      return self._process(
        template_path, 
        html.decode(self._strip(fl.read()), _default_html_config), 
        variables
      )
    } else {
      error('template "${path}" not found')
    }
  }

  _extend(base, element) {}

  _process(path, element, variables) {
    if !element return nil
  
    def error(message) {
      if !is_string(element) and !is_list(element) {
        var start = element.position.start
        raise Exception('${message} at ${path}[${start.line},${start.column}]')
      } else {
        { raise Exception(message) }
      }
    }
  
    if is_string(element) {
      return self._replace_vars(element, variables, error)
    }
  
    if is_list(element) {
      return element.map(@(el) {
        return self._process(path, el, variables)
      }).compact()
    }
    
    if element.type == 'text' {
      # replace variables: {{var_name}}
      element.content = self._process(path, element.content, variables)
    } else {
      
      # --------------------- ELEMENT PROCESSING -----------------------------

      var attrs = self._get_attrs(element.attributes)
  
      if element {

        # Process <include /> element
        if element.name == constants.INCLUDE_TAG {
          if !attrs or !attrs.contains(constants.PATH_ATTR)
            error('missing "${constants.PATH_ATTR}" attribute for ${constants.INCLUDE_TAG} tag')

          element = self._get_template_content(attrs[constants.PATH_ATTR])
        } 
        
        # Process <extend /> element
        else if element.name == constants.EXTEND_TAG {
          if !attrs or !attrs.contains(constants.BASE_ATTR)
            error('missing "${constants.BASE_ATTR}" attribute for ${constants.EXTEND_TAG} tag')

          var base_element = self._get_template_content(attrs[constants.BASE_ATTR])
          # TODO: COMPLETE
        } 
        
        # Process custom elements
        else if self._elements.contains(element.name) {
          var processed = self._elements[element.name](self, element)
          if processed {
            if !is_string(processed) {
              if is_dict(processed) and processed != element {
                element = self._process(path, processed, variables)
              } else if processed != element {
                error('invalid return when processing "${element.name}" tag')
              }
            } else {
              element = self._process(
                path, 
                html.decode(self._strip(processed), _default_html_config), 
                variables
              )
            }
          } else {
            element = nil
          }
        }
      }

      # --------------------- ATTRIBUTE PROCESSING ----------------------------
  
      # process directives
      if attrs.contains(constants.IF_ATTR) {
        # if attribute
        var _var = self._extract_var(variables, attrs.get(constants.IF_ATTR), error)
        if _var {
          self._strip_attr(element, constants.IF_ATTR)
          element = self._process(path, element, variables)
        } else {
          element = nil
        }
      } else if attrs.contains(constants.NOT_ATTR) {
        # if not attribute
        var _var = self._extract_var(variables, attrs.get(constants.NOT_ATTR), error)
        if !_var {
          self._strip_attr(element, constants.NOT_ATTR)
          element = self._process(path, element, variables)
        } else {
          element = nil
        }
      } else if attrs.contains(constants.FOR_ATTR) {
        # for attribute
        
        var data = self._extract_var(variables, attrs.get(constants.FOR_ATTR), error),
            key_name = attrs.get(constants.KEY_ATTR),
            value_name = attrs.get(constants.VALUE_ATTR, nil)
  
        self._strip_attr(
          element, constants.FOR_ATTR, 
          constants.KEY_ATTR, constants.VALUE_ATTR
        )
        var for_vars = variables.clone()
  
        var result = []
        if data {
          for key, value in data {
            if value_name for_vars.set('${value_name}', value)
            if key_name for_vars.set('${key_name}', key)

            result.append(
              self._process(path, element.clone(), for_vars)
            )
          }
        }
        return result
      }
      
      if is_dict(element) and element.get('children', nil) {
        element.children = self._process(path, element.children, variables)
      }
  
      # replace attribute variables...
      if element and !is_list(element) {
        for attr in element.attributes {
          if attr.value {
            # replace variables: {var_name}
            attr.value = self._process(path, attr.value, variables)
          }
        }
      }
    }

    return element
  }

  /**
   * Set the template files root directory for [[Template.render]]. Returns `true` if 
   * the directory was automatically created (See [[Template._auto_init]]) or `false` 
   * if it wasn't.
   * 
   * If your template contains or will contain an `<include />` tag, the path given 
   * here will become the root of the include search path.
   * 
   * @param string path
   * @returns bool
   */
  set_root(path) {
    if !is_string(path)
      raise Exception('string expected in argument 1 (path)')
    
    var directory_created = false

    if !os.dir_exists(path) and self._auto_init {
      directory_created = os.create_dir(path)
    }

    self._root_dir = path
    return directory_created
  }

  /**
   * Sets the default file extension to be used when [[Template.render]] and/or the 
   * `<include />` tag searches for template files in the root directory when the path 
   * given does not match an existing file and does not end with another extension.
   * 
   * @param string ext
   */
  set_extension(ext) {
    if !is_string(ext)
      raise Exception('string expected at argument 1 (ext)')
    if !ext.starts_with('.')
      raise Exception('invalid extension name')
    self._file_ext = ext
  }

  /**
   * Registers a function that can be used to process variables in the template. 
   * The given function must accept a minimum of one argument which will receive
   * the value of the value to be processed and at most two arguments, the second of 
   * which will receive arguments passed to the function as a string.
   * 
   * ##### Example
   * 
   * ```blade
   * def firstname_function(value) {
   *   return value.split(' ')[0]
   * }
   * 
   * tpl.register_function('firstname', firstname_function)
   * ```
   * 
   * The registered function can be used in the template to process variables.
   * For example,
   * 
   * ```wire
   * <div>{{ my_user|firstname }}</div>
   * ```
   * 
   * @param string name
   * @param function function
   */
  register_function(name, function) {
    if !is_string(name)
      raise Exception('string expected in argument 1 (name)')
    if !is_function(function)
      raise Exception('function expected in argument 1 (function)')

    var fn_arity = reflect.get_function_metadata(function).arity
    if fn_arity > 2
      raise Exception('invalid template function')
    
    self._functions.set(name, function)
  }

  /**
   * Registers a custom HTML element for the template. The function passed must 
   * take exactly two (2) arguments the first of which will receive the the
   * template object itself and the second the HTML as an object of {{html}}.
   * 
   * ##### Example
   * 
   * ```blade
   * def inline_input(wire, value) {
   *   return ...
   * }
   * 
   * tpl.register_element('inline-input', my_custom_function)
   * ```
   * 
   * The above registered element can then be used in the template. For example,
   * 
   * ```wire
   * <inline-input value="{{ my_var }}" />
   * ```
   * 
   * @param string name
   * @param function(2) element
   */
  register_element(name, element) {
    if !is_string(name)
      raise Exception('string expected in argument 1 (name)')
    if !is_function(element)
      raise Exception('function expected in argument 1 (function)')
    if reflect.get_function_metadata(element).arity != 2
      raise Exception('invalid function argument count for template element: ${name}')

    self._elements.set(name, element)
  }

  /**
   * Process and render template contained in the given string. The variables 
   * dictionary is used to pass variable data to the template being processed. 
   * 
   * If a variable is required in the template and is missing in the variables 
   * dictionary or the variables dictionary was not passed to the `render_string()` 
   * call, the process dies with an Exception. The third argument allows specifying 
   * the source file/path of the template being processed and will default to 
   * `<source>` when not passed.
   * 
   * The path argument may be of important if the string was read from a file or a 
   * similar source to provide information on the source of wrong template data such as 
   * line and column information.
   * 
   * ##### Example
   * 
   * ```blade
   * tpl.render_string('<div>{{ name }}</div>', {name: 'Johnson'})
   * ```
   * 
   * The above template should return
   * 
   * ```wire
   * <div>Johnson</div>
   * ```
   * 
   * @param string source
   * @param dict? variables
   * @param string? path
   * @returns string
   */
  render_string(source, variables, path) {
    if !is_string(source)
      raise Exception('template template expected')
  
    if variables != nil and !is_dict(variables)
      raise Exception('variables must be passed to render_string() as a dictionary')
    if variables == nil variables = {}

    if !path path = '<source>'
    else path = to_string(path)
  
    return html.encode(
      self._process(
        path,
        html.decode(self._strip(source), _default_html_config),
        variables
      )
    ).trim()
  }

  /**
   * Process and render template contained in the given template file. The template 
   * path should be a path relative to the root directory (See [[Template]]) and may 
   * or not carry any extension. If the template file uses the template _extension_ 
   * (default: `.html`), the path argument may exclude the extension from the path
   * altogether provided there is a file with a matching name that may or not have the 
   * default extension (See [[Template.set_extension]]). 
   * 
   * The variables dictionary is used to pass variable data to the template being 
   * processed and behaves exactly the same way as with [[Template.render_string]].
   * 
   * ##### Example
   * 
   * ```blade
   * tpl.render('my_template')
   * ```
   * 
   * The above example renders the template as is and will raise if any variable is found in it.
   * You can pass a variable the same way you do with [[Template.render_string]].
   * 
   * @param string path
   * @param dict? variables
   * @returns string
   */
  render(path, variables) {
    if !is_string(path)
      raise Exception('template path expected')

    # confirm/auto create root directory as configured
    if !os.dir_exists(self._root_dir) {
      if !self._auto_init 
        raise Exception('templates directory "${self._root_dir}" not found.')
      else os.create_dir(self._root_dir)
    }
  
    var template_path = os.join_paths(self._root_dir, path)
    if !file(template_path).exists() {
      if !template_path.match(constants.EXT_RE) 
        template_path += self._file_ext
    }
  
    if variables != nil and !is_dict(variables)
      raise Exception('variables must be passed to render() as a dictionary')
    if variables == nil variables = {}
  
    var template_file = file(template_path)
    if template_file.exists() {
      return self.render_string(template_file.read(), variables, template_path)
    }
  
    raise Exception('template "${path}" not found')
  }

  /**
   * Same as [[template.attr]], but available as a method on the template instance for easier access 
   * in custom element functions during element registration.
   * 
   * @param dict element
   * @param string name
   * @returns dict?
   */
  attr(element, name) {
    return attr(element, name)
  }
}


/**
 * Default function exporting the [[Template]] class that allows function 
 * initialization. See [[Template]].
 * 
 * @param bool auto_init
 * @default
 */
def template(auto_init) {
  return Template(auto_init)
}

