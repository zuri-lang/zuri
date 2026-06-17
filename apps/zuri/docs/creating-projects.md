# Creating Projects

Nyssa projecs are typical Zuri  applications

### To create a new project

1. Create a new directory for your project and open a terminal/command window in the directory and run the `nyssa init` command.
2. Fill in the responses to the prompted questions. `name` and `version` are required so nyssa will suggest an appropriate value for you. You can leave them blank to allow nyssa use the default values or replace them with other values of your choice.
  
  See [Package layout](/docs/package-layout) for the structure of the command output.

  > The default `name` will be the name of current directory and the default version will be `1.0.0`. It is conventional to keep the version name semantic.

### Converting an existing project

To convert an existing application or library into a Nyssa application, all you need to do is to follow the same process for creating a new package. **If the application already contains a file named `project.json`, you may need to delete it** because nyssa does not reinitialize existing projects.

_When converting an existing application, your existing directory and files matching those that nyssa creates will not be overwritten_.

### Where to write code

Your code can go into any Zuri (`.zu`) file in the created/generated directories and files. However, by default and by convention, your code should go into the `app` directory.

### Running your application

To run your application, run the command `zuri .` from the root of the application.

When you create a new nyssa project, nyssa creates a simple Hello World like application for you as well. If you run `zuri .` from the root of your new nyssa project, you'll be greeted by the message `Welcome to Nyssa. Magic begins here!`. To modify this, you can open the file at `app/index.zu` to change what the application does.

