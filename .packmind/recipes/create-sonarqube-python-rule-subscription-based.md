Create an event-driven code quality rule for the SonarQube Python plugin using PythonSubscriptionCheck, with comprehensive tests and metadata for production deployment.

## When to Use

- Implementing simple checks targeting specific AST node types (comparison, call expression, function definition)
- Creating rules with straightforward detection logic without complex traversal
- Adding new code quality, bug detection, or security rules to the python-checks module
- Extending the SonarQube Python plugin with custom validation patterns
- Building rules that analyze isolated code patterns rather than code flow

## Context Validation Checkpoints

* [ ] What AST node types will the rule analyze (e.g., comparison, call expression, function definition)?
* [ ] What specific violation pattern needs to be detected and what is the error message?
* [ ] What is the rule's type (CODE_SMELL, BUG, VULNERABILITY), impact level (HIGH/MEDIUM/LOW), and severity (Critical, Major, Minor)?
* [ ] Is there an available rule key (SXXXX format) assigned for this rule?
* [ ] Does the rule need quick fixes or secondary issue locations to aid developers?

## Recipe Steps

### Step 1: Create Rule Class with Subscription Pattern

Create a new Java class in `python-checks/src/main/java/org/sonar/python/checks/` extending `PythonSubscriptionCheck` with the `@Rule` annotation. Register AST node consumers in the `initialize()` method using `context.registerSyntaxNodeConsumer()`.

```java
@Rule(key = "S5727")
public class ComparisonToNoneCheck extends PythonSubscriptionCheck {

  @Override
  public void initialize(Context context) {
    context.registerSyntaxNodeConsumer(Kind.COMPARISON,
      ctx -> checkComparison(ctx, (BinaryExpression) ctx.syntaxNode()));
  }
}
```

### Step 2: Implement Detection Logic

Add private methods that analyze the registered AST nodes and report issues using `ctx.addIssue(tree, message)`. Use type inference and utility methods from `CheckUtils` to detect violations.

```java
private static void checkComparison(SubscriptionContext ctx, BinaryExpression comparison) {
  InferredType left = comparison.leftOperand().type();
  InferredType right = comparison.rightOperand().type();

  if (isNone(left) && isNone(right)) {
    ctx.addIssue(comparison, "Remove this comparison; it will always be True.");
  }
}
```

### Step 3: Create Test Class

Create test class in `python-checks/src/test/java/org/sonar/python/checks/` named `<RuleName>CheckTest`. Use `PythonCheckVerifier.verify()` to validate rule behavior against test resources.

```java
class ComparisonToNoneCheckTest {

  @Test
  void test() {
    PythonCheckVerifier.verify(
      "src/test/resources/checks/comparisonToNoneCheck.py",
      new ComparisonToNoneCheck()
    );
  }
}
```

### Step 4: Create Test Resource File

Create Python file in `python-checks/src/test/resources/checks/` with compliant and non-compliant examples. Use `# Noncompliant {{message}}` annotations to mark expected violations.

```python
def check_none(param):
    a = None

    if a is None: pass  # Noncompliant {{Remove this check; it will always be True.}}
    if param is None: pass  # OK - param could be None
```

### Step 5: Create JSON Metadata

Create `<RuleKey>.json` in `sonar-python-plugin/src/main/resources/org/sonar/l10n/py/rules/python/` with rule configuration including type, impacts, severity, and remediation cost.

```json
{
  "title": "Comparison to None should not be constant",
  "type": "CODE_SMELL",
  "code": {
    "impacts": {"MAINTAINABILITY": "HIGH"},
    "attribute": "LOGICAL"
  },
  "status": "ready",
  "remediation": {"func": "Constant/Issue", "constantCost": "10min"},
  "defaultSeverity": "Critical",
  "sqKey": "S5727"
}
```

### Step 6: Create HTML Description

Create `<RuleKey>.html` with rule documentation including rationale, code examples with compliant/noncompliant pairs, and resource links. Follow the standard structure: issue explanation, code examples, and resources.

```html
<h2>Why is this an issue?</h2>
<p>Checking if a variable is None should only be done when it can be None...</p>

<h3>Code examples</h3>
<h4>Noncompliant code example</h4>
<pre>my_var = None
if my_var == None:  # Always True</pre>

<h4>Compliant solution</h4>
<pre>def foo(my_var):
    if my_var == None:  # my_var could be None</pre>
```

### Step 7: Register Rule in CheckList

Add the rule class to `OpenSourceCheckList.getChecks()` in alphabetical order. Import the class at the top of the file.

```java
public Stream<Class<?>> getChecks() {
  return Stream.of(
    ArgumentNumberCheck.class,
    ComparisonToNoneCheck.class,  // Add here alphabetically
    HardcodedIPCheck.class,
    // ...
  );
}
```

### Step 8: Verify and Build

Run tests to verify implementation, format license headers with `mvn license:format`, and ensure code coverage meets requirements. Build the project to confirm integration.

```bash
mvn test -Dtest=ComparisonToNoneCheckTest
mvn license:format
mvn clean install -DskipTypeshed -DskipTests
```
