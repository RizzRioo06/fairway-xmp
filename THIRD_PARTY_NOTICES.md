# Third-party software

Fairway bundles independently licensed software. The Fairway application code is MIT licensed; that license does not replace the licenses of bundled components.

| Component | License / upstream |
|---|---|
| PHP | PHP License 3.01 and Zend Engine license; https://www.php.net/license/ |
| MySQL Community Server | GPL v2 with Universal FOSS Exception; https://github.com/mysql/mysql-server |
| Nginx | BSD 2-Clause; https://nginx.org/LICENSE |
| phpMyAdmin | GPL v2 or later; https://github.com/phpmyadmin/phpmyadmin |
| Composer | MIT; https://github.com/composer/composer |
| Tauri | MIT / Apache 2.0; https://github.com/tauri-apps/tauri |
| React | MIT; https://github.com/facebook/react |
| Lucide | ISC; https://github.com/lucide-icons/lucide |

Runtime staging retains upstream license, notice, and copying files under `runtime/licenses` and within component directories. Microsoft Visual C++ and WebView2 are separately licensed prerequisites and remain on uninstall if shared with other applications.

Release distributors must accompany GPL binaries with the corresponding source code, including any packaging patches and dependency sources required by the applicable licenses. `runtime-downloads.lock.json` records exact binary inputs. Do not treat an upstream project link alone as fulfillment of the source distribution obligation. Include a corresponding-source archive alongside future binary releases.
