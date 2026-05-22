/**
 * This example sends an email with itself as a file attachment.
 */

# Change the MY_* to their correct values.

import mail

var smtp = mail.smtp({
  host: 'MY_SMTP_HOST', # e.g. smtp.gmail.com
  username: 'MY_SMTP_USERNAME', # usually your email address
  password: 'MY_SMTP_PASSWORD',
})

echo smtp.add_message(mail.message().
  to('MY_MAIL_RECIPIENT'). # change to the correct value
  subject('Testing a very new transmission').
  html(
    'Hi,'+
    '<br><br>'+
    'If you are receiving this mail, know that it was sent '+
    'with the <strong>mailer</strong> library for '+
    '<a href="https://zurilang.org">Zuri programming language</a>.' +
    '<br><br>' +
    'Sincerely,<br>' +
    'The Zuri Community'
  ).
  attachment(__file__, 'smtp_mail.b')
).send()
