RELEASE_TYPE: patch

This patch adds a weighted generator for booleans, `booleans(double p)`.

```java
booleans(1.0) // always true
booleans(0.7) // 70% probability of true
booleans(0.0) // always false
```