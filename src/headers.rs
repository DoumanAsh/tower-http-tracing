use core::fmt;

struct DisplayHeaderValues<'a>(http::header::GetAll<'a, http::header::HeaderValue>);

impl fmt::Debug for DisplayHeaderValues<'_> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        const FALLBACK_STR: &str = "<non-utf8>";

        let mut headers = self.0.iter();
        if let Some(header) = headers.next() {
            match header.to_str() {
                Ok(header) => fmt.write_str(header)?,
                Err(_) => fmt.write_str(FALLBACK_STR)?,
            }

            for header in headers {
                fmt.write_str(" ,")?;
                match header.to_str() {
                    Ok(header) => fmt.write_str(header)?,
                    Err(_) => fmt.write_str(FALLBACK_STR)?,
                }
            }
        }

        Ok(())
    }
}

pub struct InspectHeaders<'a> {
    pub header_list: &'a [&'a http::HeaderName],
    pub headers: &'a http::HeaderMap,
}

impl fmt::Debug for InspectHeaders<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = fmt.debug_map();
        for key in self.header_list {
            let all_values = self.headers.get_all(*key);
            if all_values.iter().next().is_some() {
                out.entry(&key.as_str(), &DisplayHeaderValues(all_values));
            }
        }

        out.finish()
    }
}
