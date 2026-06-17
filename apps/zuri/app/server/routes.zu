import .api
import .views
import .statics

var routes = {
  # api endpoints
  '/static/*': ['GET', statics.static_handler],
  '/source/*': ['GET', statics.source_handler],
  '/api/packages': ['GET', api.all_package],
  '/api/create-package': ['POST', api.create_package],
  '/api/get-package/{name}': ['GET', api.get_package],
  '/api/create-publisher': ['POST', api.create_publisher],
  '/api/login': ['POST', api.login],

  # frontend website pages
  '/': ['GET', views.home],
  '/404': ['GET', views.error_page],
  '/login': ['GET', views.login],
  '/forgot-password': ['GET', views.password_recovery],
  '/change-password': ['GET', views.change_password],
  '/auth': ['POST', views.authenticate],
  '/recover': ['POST', views.recover],
  '/update-password': ['POST', views.update_password],
  '/account': ['GET', views.account],
  '/search': ['GET', views.search],
  '/view/{id}': ['GET', views.view],
  '/revert': ['POST', views.revert],
  '/archive/{name}': ['GET', views.archive],
  '/logout': ['GET', views.logout],
#   '/docs': ['GET', views.doc],
#   '/docs/*': ['GET', views.doc],
}
