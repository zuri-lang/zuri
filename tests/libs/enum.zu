import enum

# list initialization
var Gender = enum(['Male', 'Female'])
echo to_string(Gender)

echo Gender.Male
echo Gender.Female

# dict initialization
var Color = enum({
  Red: 'r',
  Green: 'g',
  Blue: 'b',
})
echo to_string(Color)

echo Color.Red
echo Color.Green
echo Color.Blue

# list initialization with duplicates disallowed
catch {
  var NoDuplicates = enum({
    Slow: 1,
    Sluggish: 1,
    Fast: 2,
  })
} as dup_error

if dup_error {
  echo dup_error.message
}

# list initialization with duplicates allowed
var Speed = enum({
  Slow: 1,
  Sluggish: 1,
  Fast: 2,
}, false)
echo to_string(Speed)

# keys test
echo enum.keys(Color)
echo enum.values(Gender)
echo enum.to_value_dict(Speed)

var Holiday = enum([
  'Christmas',
  'Easter',
  'NewYear'
])

echo enum.has(Holiday, 'NineEleven')
echo enum.has(Holiday, 'Easter')
