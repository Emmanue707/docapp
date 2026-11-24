-dontobfuscate
-keepattributes SourceFile,LineNumberTable

-dontwarn java.awt.**
-dontwarn java.beans.**

-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.Library { *; }
-keepclassmembers class * extends com.sun.jna.Structure { *; }

-keep class uniffi.docsuite_ffi.** { *; }
-keep interface uniffi.docsuite_ffi.** { *; }