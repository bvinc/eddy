+use tokio::runtime::Runtime;
 
 mod color;
 mod components;
@@ -61,6 +62,18 @@ fn main() -> ExitCode {
     //     }),
     // );
 
+    // Start the Tokio runtime in a separate thread
+    std::thread::spawn(move || {
+        let rt = Runtime::new().unwrap();
+        rt.block_on(async move {
+            // Your asynchronous code here
+            glib::idle_add_local(move || {
+                println!("Message from Tokio runtime!");
+                ControlFlow::Break
+            });
+        });
+    });
+
