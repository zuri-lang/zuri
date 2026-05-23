var can = {
  name: 'pamplemousse',
  ounces: 12,
}

describe('the can', @{
  it('has 12 ounces', @{
    expect(can.ounces).to_be(12)

    class A {
      var name = 'something'
    }
    
    expect(A()).to_have_property('name')
    expect(A()).to_have_property('name', 'something')

    expect(10.5).to_be_number().to_be_less_than(20)
  })

  it('has a sophisticated name', @{
    expect(can.name).to_be('pamplemousse')

    expect([1, 2, 3]).to_have_length(3)
    expect('abc').to_have_length(3)
    expect('').not().to_have_length(5)
  })

  it('should pass this tests', @{
    class A {
      testing() {}
      @testing() {}
    }

    expect(A()).to_be_instance_of(A)
    # expect(A()).to_be_instance_of(Exception) # fails
    expect(A()).to_have_method('testing')

    def bloop() {
      return nil
    }
    
    expect(bloop()).to_be_nil()

    expect(A()).to_have_decorator('testing')
  })
})

def do_something(id) {
  if id == 1 return @{ do_another_thing() }
  else return @{ do_something_else() }
}

class Set {
  @iter() {}
  @itern() {}
}

describe('grapefruits', @{
  it('should be a grape', @{
    expect('grapefruits').to_match('grape')
    expect(do_something(1)).to_be_function()

    expect('Hosana').to_be_string()
    expect(Exception).to_be_class()
  })
  
  it('should be valid type', @{
    expect({age: 10}).to_be_dict()
    expect(bytes(0)).to_be_bytes()
  })

  it('should be enumerable', @{
    expect([]).to_be_iterable()
    expect({}).to_be_iterable()
    expect(Set()).to_be_iterable()
  })
})

class DisgustingFlavorError < Exception {}

def drink_flavor(flavor) {
  if flavor == 'octopus' {
    raise DisgustingFlavorError('yuck, octopus flavor')
  }
  # Do some other stuff
}

describe('testing to throw', @{
  it('throws on octopus', @{
    def drink_octopus() {
      drink_flavor('octopus')
    }
  
    # Test that the error message says "yuck" somewhere: these are equivalent
    expect(drink_octopus).to_throw('/yuck/')
    expect(drink_octopus).to_throw('yuck')
  
    # Test the exact error message
    expect(drink_octopus).to_throw('/^yuck, octopus flavor$/')
    expect(drink_octopus).to_throw(Exception('yuck, octopus flavor'))
  
    # Test that we get a DisgustingFlavorError
    expect(drink_octopus).to_throw(DisgustingFlavorError)
  })
})
