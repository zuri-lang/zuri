var myBeverage = {
  delicious: true,
  sour: false,
}

describe('my beverage', @{
  it('should be delicious', @{
    expect(myBeverage.delicious).to_be_truthy()
  });

  it('should be sour', @{
    expect(myBeverage.sour).to_be_falsy()
  })
})

class CustomError < Exception {}

var binay_string_to_number = @( bin_string ) {
  if !bin_string.match('/^[01]+$/') {
    raise CustomError('Not a binary number.')
  }

  return to_number('0b' + bin_string)
}

describe('binay string to number', @{
  describe('given an invalid binary string', @{
    it('throws CustomError when composed of non-numbers', @{
      expect(@{ binay_string_to_number('abc') }).to_throw(CustomError)
    })

    it('throws CustomError when having extra whitespace', @{
      expect(@{ binay_string_to_number('  100') }).to_throw(CustomError)
    })
  })

  describe('given a valid binary string', @{
    it('returns the correct number', @{
      expect(binay_string_to_number('100')).to_be(4)
    })
  })
})
